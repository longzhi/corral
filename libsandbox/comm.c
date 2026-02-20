#include "comm.h"
#include "cJSON.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <pthread.h>
#include <errno.h>

static int g_broker_fd = -1;
static pthread_mutex_t g_broker_mutex = PTHREAD_MUTEX_INITIALIZER;
static int g_request_id = 1;

bool broker_init(void) {
    pthread_mutex_lock(&g_broker_mutex);
    
    if (g_broker_fd >= 0) {
        pthread_mutex_unlock(&g_broker_mutex);
        return true;
    }
    
    const char *socket_path = getenv("SANDBOX_SOCKET");
    if (!socket_path) {
        /* No broker socket configured */
        pthread_mutex_unlock(&g_broker_mutex);
        return false;
    }
    
    /* Create Unix socket */
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) {
        pthread_mutex_unlock(&g_broker_mutex);
        return false;
    }
    
    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, socket_path, sizeof(addr.sun_path) - 1);
    
    if (connect(fd, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        close(fd);
        pthread_mutex_unlock(&g_broker_mutex);
        return false;
    }
    
    g_broker_fd = fd;
    pthread_mutex_unlock(&g_broker_mutex);
    return true;
}

void broker_cleanup(void) {
    pthread_mutex_lock(&g_broker_mutex);
    
    if (g_broker_fd >= 0) {
        close(g_broker_fd);
        g_broker_fd = -1;
    }
    
    pthread_mutex_unlock(&g_broker_mutex);
}

/* Helper: Send JSON-RPC request and get response */
static policy_decision_t broker_request(const char *method, cJSON *params) {
    pthread_mutex_lock(&g_broker_mutex);
    
    if (g_broker_fd < 0) {
        pthread_mutex_unlock(&g_broker_mutex);
        return POLICY_DENY;
    }
    
    /* Build JSON-RPC request */
    cJSON *request = cJSON_CreateObject();
    cJSON_AddStringToObject(request, "jsonrpc", "2.0");
    cJSON_AddNumberToObject(request, "id", g_request_id++);
    cJSON_AddStringToObject(request, "method", method);
    if (params) {
        cJSON_AddItemToObject(request, "params", params);
    }
    
    char *request_str = cJSON_PrintUnformatted(request);
    cJSON_Delete(request);
    
    if (!request_str) {
        pthread_mutex_unlock(&g_broker_mutex);
        return POLICY_DENY;
    }
    
    /* Send request (with newline for JSON Lines format) */
    size_t len = strlen(request_str);
    char *msg = malloc(len + 2);
    sprintf(msg, "%s\n", request_str);
    free(request_str);
    
    ssize_t written = write(g_broker_fd, msg, len + 1);
    free(msg);
    
    if (written < 0) {
        pthread_mutex_unlock(&g_broker_mutex);
        return POLICY_DENY;
    }
    
    /* Read response (simple line-based read) */
    char response_buf[4096];
    ssize_t n = read(g_broker_fd, response_buf, sizeof(response_buf) - 1);
    
    pthread_mutex_unlock(&g_broker_mutex);
    
    if (n <= 0) {
        return POLICY_DENY;
    }
    
    response_buf[n] = '\0';
    
    /* Parse response */
    cJSON *response = cJSON_Parse(response_buf);
    if (!response) {
        return POLICY_DENY;
    }
    
    /* Check for error */
    cJSON *error = cJSON_GetObjectItem(response, "error");
    if (error) {
        cJSON_Delete(response);
        return POLICY_DENY;
    }
    
    /* Get result */
    cJSON *result = cJSON_GetObjectItem(response, "result");
    cJSON *allowed = cJSON_GetObjectItem(result, "allowed");
    
    policy_decision_t decision = POLICY_DENY;
    if (cJSON_IsBool(allowed) && cJSON_IsTrue(allowed)) {
        decision = POLICY_ALLOW;
    }
    
    cJSON_Delete(response);
    return decision;
}

policy_decision_t broker_ask_path(const char *path, operation_t op) {
    if (!path) return POLICY_DENY;
    
    cJSON *params = cJSON_CreateObject();
    cJSON_AddStringToObject(params, "path", path);
    
    const char *op_str = "read";
    switch (op) {
        case OP_WRITE: op_str = "write"; break;
        case OP_EXEC: op_str = "exec"; break;
        default: op_str = "read"; break;
    }
    cJSON_AddStringToObject(params, "operation", op_str);
    
    return broker_request("sandbox.check_path", params);
}

policy_decision_t broker_ask_network(const char *host, int port) {
    if (!host) return POLICY_DENY;
    
    cJSON *params = cJSON_CreateObject();
    cJSON_AddStringToObject(params, "host", host);
    cJSON_AddNumberToObject(params, "port", port);
    
    return broker_request("sandbox.check_network", params);
}

policy_decision_t broker_ask_exec(const char *path) {
    if (!path) return POLICY_DENY;
    
    cJSON *params = cJSON_CreateObject();
    cJSON_AddStringToObject(params, "path", path);
    
    return broker_request("sandbox.check_exec", params);
}

policy_decision_t broker_ask_dlopen(const char *path) {
    if (!path) return POLICY_DENY;
    
    cJSON *params = cJSON_CreateObject();
    cJSON_AddStringToObject(params, "path", path);
    
    return broker_request("sandbox.check_dlopen", params);
}
