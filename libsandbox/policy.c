#include "policy.h"
#include "cJSON.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fnmatch.h>
#include <pthread.h>
#include <unistd.h>

/* Global policy structure */
typedef struct {
    char **read_paths;
    size_t read_count;
    char **write_paths;
    size_t write_count;
    char **exec_paths;
    size_t exec_count;
    char **network_hosts;
    size_t network_count;
    bool initialized;
} sandbox_policy_t;

static sandbox_policy_t g_policy = {0};
static pthread_mutex_t g_policy_mutex = PTHREAD_MUTEX_INITIALIZER;

/* Helper: Parse string array from JSON */
static char** parse_string_array(cJSON *array, size_t *count) {
    if (!cJSON_IsArray(array)) {
        *count = 0;
        return NULL;
    }
    
    int size = cJSON_GetArraySize(array);
    if (size == 0) {
        *count = 0;
        return NULL;
    }
    
    char **result = calloc(size, sizeof(char*));
    if (!result) {
        *count = 0;
        return NULL;
    }
    
    int idx = 0;
    cJSON *item = NULL;
    cJSON_ArrayForEach(item, array) {
        if (cJSON_IsString(item) && item->valuestring) {
            result[idx] = strdup(item->valuestring);
            idx++;
        }
    }
    
    *count = idx;
    return result;
}

/* Helper: Free string array */
static void free_string_array(char **array, size_t count) {
    if (!array) return;
    for (size_t i = 0; i < count; i++) {
        free(array[i]);
    }
    free(array);
}

/* Helper: Check if path matches any pattern in the list */
static bool match_path_list(const char *path, char **patterns, size_t count) {
    if (!path || !patterns) return false;
    
    for (size_t i = 0; i < count; i++) {
        if (!patterns[i]) continue;
        
        /* Support simple glob patterns with fnmatch */
        if (fnmatch(patterns[i], path, 0) == 0) {
            return true;
        }
    }
    
    return false;
}

/* Helper: Check if host matches any pattern in the list */
static bool match_host_list(const char *host, int port, char **patterns, size_t count) {
    if (!host || !patterns) return false;
    
    char hostport[512];
    snprintf(hostport, sizeof(hostport), "%s:%d", host, port);
    
    for (size_t i = 0; i < count; i++) {
        if (!patterns[i]) continue;
        
        /* Match exact host or host:port */
        if (strcmp(patterns[i], host) == 0 || strcmp(patterns[i], hostport) == 0) {
            return true;
        }
        
        /* Support wildcard patterns */
        if (fnmatch(patterns[i], host, 0) == 0 || fnmatch(patterns[i], hostport, 0) == 0) {
            return true;
        }
    }
    
    return false;
}

/* Helper: Load policy from file descriptor (Linux only) */
#ifdef __linux__
static char* load_policy_from_fd(int fd) {
    if (fd < 0) return NULL;
    
    /* Read from /proc/self/fd/{fd} */
    char fd_path[64];
    snprintf(fd_path, sizeof(fd_path), "/proc/self/fd/%d", fd);
    
    FILE *f = fopen(fd_path, "r");
    if (!f) return NULL;
    
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    
    if (size <= 0 || size > 10*1024*1024) { /* Max 10MB policy */
        fclose(f);
        return NULL;
    }
    
    char *content = malloc(size + 1);
    if (!content) {
        fclose(f);
        return NULL;
    }
    
    size_t read_bytes = fread(content, 1, size, f);
    content[read_bytes] = '\0';
    fclose(f);
    
    return content;
}
#endif

/* Helper: Load policy from file path */
static char* load_policy_from_file(const char *path) {
    if (!path) return NULL;
    
    FILE *f = fopen(path, "r");
    if (!f) return NULL;
    
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    
    if (size <= 0 || size > 10*1024*1024) { /* Max 10MB policy */
        fclose(f);
        return NULL;
    }
    
    char *content = malloc(size + 1);
    if (!content) {
        fclose(f);
        return NULL;
    }
    
    size_t read_bytes = fread(content, 1, size, f);
    content[read_bytes] = '\0';
    fclose(f);
    
    /* Security: Immediately unlink the file after reading (read-once semantics) */
    unlink(path);
    
    return content;
}

/* Helper: Zero out memory buffer (prevent memory dumps) */
static void secure_zero(void *ptr, size_t len) {
    if (!ptr) return;
    volatile unsigned char *p = (volatile unsigned char *)ptr;
    while (len--) {
        *p++ = 0;
    }
}

bool policy_init(void) {
    pthread_mutex_lock(&g_policy_mutex);
    
    if (g_policy.initialized) {
        pthread_mutex_unlock(&g_policy_mutex);
        return true;
    }
    
    char *policy_json_raw = NULL;
    cJSON *root = NULL;
    
    /* Try loading policy in order of security preference:
     * 1. SANDBOX_POLICY_FD (Linux only, most secure - memfd)
     * 2. SANDBOX_POLICY_FILE (cross-platform, secure - temp file)
     * 3. SANDBOX_POLICY (env var, fallback - less secure)
     */
    
#ifdef __linux__
    const char *fd_str = getenv("SANDBOX_POLICY_FD");
    if (fd_str) {
        int fd = atoi(fd_str);
        policy_json_raw = load_policy_from_fd(fd);
        /* Clear env var for security */
        unsetenv("SANDBOX_POLICY_FD");
    }
#endif
    
    if (!policy_json_raw) {
        const char *policy_file = getenv("SANDBOX_POLICY_FILE");
        if (policy_file) {
            policy_json_raw = load_policy_from_file(policy_file);
            /* Clear env var for security */
            unsetenv("SANDBOX_POLICY_FILE");
        }
    }
    
    if (!policy_json_raw) {
        const char *policy_env = getenv("SANDBOX_POLICY");
        if (policy_env) {
            policy_json_raw = strdup(policy_env);
            /* Clear env var for security */
            unsetenv("SANDBOX_POLICY");
        }
    }
    
    if (!policy_json_raw) {
        /* No policy found - deny everything */
        g_policy.initialized = true;
        pthread_mutex_unlock(&g_policy_mutex);
        return true;
    }
    
    /* Parse JSON policy */
    root = cJSON_Parse(policy_json_raw);
    
    /* Security: Zero out the raw JSON buffer after parsing */
    size_t json_len = strlen(policy_json_raw);
    secure_zero(policy_json_raw, json_len);
    free(policy_json_raw);
    policy_json_raw = NULL;
    
    if (!root) {
        /* Invalid JSON - deny everything */
        g_policy.initialized = true;
        pthread_mutex_unlock(&g_policy_mutex);
        return true;
    }
    
    /* Parse fs.read */
    cJSON *fs = cJSON_GetObjectItem(root, "fs");
    if (fs) {
        cJSON *read = cJSON_GetObjectItem(fs, "read");
        g_policy.read_paths = parse_string_array(read, &g_policy.read_count);
        
        cJSON *write = cJSON_GetObjectItem(fs, "write");
        g_policy.write_paths = parse_string_array(write, &g_policy.write_count);
    }
    
    /* Parse exec */
    cJSON *exec = cJSON_GetObjectItem(root, "exec");
    g_policy.exec_paths = parse_string_array(exec, &g_policy.exec_count);
    
    /* Parse network.allow */
    cJSON *network = cJSON_GetObjectItem(root, "network");
    if (network) {
        cJSON *allow = cJSON_GetObjectItem(network, "allow");
        g_policy.network_hosts = parse_string_array(allow, &g_policy.network_count);
    }
    
    cJSON_Delete(root);
    g_policy.initialized = true;
    
    pthread_mutex_unlock(&g_policy_mutex);
    return true;
}

void policy_cleanup(void) {
    pthread_mutex_lock(&g_policy_mutex);
    
    free_string_array(g_policy.read_paths, g_policy.read_count);
    free_string_array(g_policy.write_paths, g_policy.write_count);
    free_string_array(g_policy.exec_paths, g_policy.exec_count);
    free_string_array(g_policy.network_hosts, g_policy.network_count);
    
    memset(&g_policy, 0, sizeof(g_policy));
    
    pthread_mutex_unlock(&g_policy_mutex);
}

policy_decision_t policy_check_path(const char *path, operation_t op) {
    if (!path) return POLICY_DENY;
    
    pthread_mutex_lock(&g_policy_mutex);
    
    if (!g_policy.initialized) {
        pthread_mutex_unlock(&g_policy_mutex);
        return POLICY_DENY;
    }
    
    bool allowed = false;
    
    switch (op) {
        case OP_READ:
            allowed = match_path_list(path, g_policy.read_paths, g_policy.read_count);
            break;
        case OP_WRITE:
            allowed = match_path_list(path, g_policy.write_paths, g_policy.write_count);
            break;
        case OP_EXEC:
            allowed = match_path_list(path, g_policy.exec_paths, g_policy.exec_count);
            break;
    }
    
    pthread_mutex_unlock(&g_policy_mutex);
    
    return allowed ? POLICY_ALLOW : POLICY_DENY;
}

policy_decision_t policy_check_network(const char *host, int port) {
    if (!host) return POLICY_DENY;
    
    pthread_mutex_lock(&g_policy_mutex);
    
    if (!g_policy.initialized) {
        pthread_mutex_unlock(&g_policy_mutex);
        return POLICY_DENY;
    }
    
    bool allowed = match_host_list(host, port, g_policy.network_hosts, g_policy.network_count);
    
    pthread_mutex_unlock(&g_policy_mutex);
    
    return allowed ? POLICY_ALLOW : POLICY_DENY;
}

policy_decision_t policy_check_exec(const char *path) {
    return policy_check_path(path, OP_EXEC);
}

policy_decision_t policy_check_dlopen(const char *path) {
    /* For now, dlopen uses same policy as exec */
    return policy_check_path(path, OP_EXEC);
}
