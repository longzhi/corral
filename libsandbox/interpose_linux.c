#ifdef __linux__

#define _GNU_SOURCE
#include "policy.h"
#include "comm.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <errno.h>
#include <stdarg.h>
#include <dlfcn.h>
#include <spawn.h>
#include <pthread.h>

/* Original function pointers */
static int (*real_open)(const char*, int, ...) = NULL;
static int (*real_access)(const char*, int) = NULL;
static int (*real_stat)(const char*, struct stat*) = NULL;
static int (*real_lstat)(const char*, struct stat*) = NULL;
static int (*real_unlink)(const char*) = NULL;
static int (*real_rename)(const char*, const char*) = NULL;
static int (*real_connect)(int, const struct sockaddr*, socklen_t) = NULL;
static int (*real_bind)(int, const struct sockaddr*, socklen_t) = NULL;
static int (*real_getaddrinfo)(const char*, const char*, const struct addrinfo*, struct addrinfo**) = NULL;
static int (*real_execve)(const char*, char *const[], char *const[]) = NULL;
static int (*real_posix_spawn)(pid_t*, const char*, const posix_spawn_file_actions_t*, 
                                const posix_spawnattr_t*, char *const[], char *const[]) = NULL;
static void* (*real_dlopen)(const char*, int) = NULL;

static pthread_once_t init_once = PTHREAD_ONCE_INIT;

/* Helper: Initialize real function pointers */
static void init_real_functions(void) {
    real_open = dlsym(RTLD_NEXT, "open");
    real_access = dlsym(RTLD_NEXT, "access");
    real_stat = dlsym(RTLD_NEXT, "stat");
    real_lstat = dlsym(RTLD_NEXT, "lstat");
    real_unlink = dlsym(RTLD_NEXT, "unlink");
    real_rename = dlsym(RTLD_NEXT, "rename");
    real_connect = dlsym(RTLD_NEXT, "connect");
    real_bind = dlsym(RTLD_NEXT, "bind");
    real_getaddrinfo = dlsym(RTLD_NEXT, "getaddrinfo");
    real_execve = dlsym(RTLD_NEXT, "execve");
    real_posix_spawn = dlsym(RTLD_NEXT, "posix_spawn");
    real_dlopen = dlsym(RTLD_NEXT, "dlopen");
}

/* Helper: Check path and decide */
static inline policy_decision_t check_and_decide_path(const char *path, operation_t op) {
    policy_decision_t decision = policy_check_path(path, op);
    if (decision == POLICY_ASK_BROKER) {
        decision = broker_ask_path(path, op);
    }
    return decision;
}

/* Helper: Safe realpath */
static char* safe_realpath(const char *path) {
    char *resolved = realpath(path, NULL);
    if (!resolved) {
        return strdup(path);
    }
    return resolved;
}

/*
 * File operations
 */

int open(const char *path, int flags, ...) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_open) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved = safe_realpath(path);
    operation_t op = (flags & (O_WRONLY | O_RDWR | O_CREAT | O_TRUNC)) ? OP_WRITE : OP_READ;
    policy_decision_t decision = check_and_decide_path(resolved, op);
    free(resolved);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    mode_t mode = 0;
    if (flags & O_CREAT) {
        va_list args;
        va_start(args, flags);
        mode = va_arg(args, mode_t);
        va_end(args);
        return real_open(path, flags, mode);
    }
    
    return real_open(path, flags);
}

int access(const char *path, int mode) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_access) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved = safe_realpath(path);
    operation_t op = (mode & W_OK) ? OP_WRITE : OP_READ;
    policy_decision_t decision = check_and_decide_path(resolved, op);
    free(resolved);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_access(path, mode);
}

int stat(const char *path, struct stat *buf) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_stat) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved = safe_realpath(path);
    policy_decision_t decision = check_and_decide_path(resolved, OP_READ);
    free(resolved);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_stat(path, buf);
}

int lstat(const char *path, struct stat *buf) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_lstat) {
        errno = EINVAL;
        return -1;
    }
    
    policy_decision_t decision = check_and_decide_path(path, OP_READ);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_lstat(path, buf);
}

int unlink(const char *path) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_unlink) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved = safe_realpath(path);
    policy_decision_t decision = check_and_decide_path(resolved, OP_WRITE);
    free(resolved);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_unlink(path);
}

int rename(const char *oldpath, const char *newpath) {
    pthread_once(&init_once, init_real_functions);
    
    if (!oldpath || !newpath || !real_rename) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved_old = safe_realpath(oldpath);
    char *resolved_new = safe_realpath(newpath);
    
    policy_decision_t decision_old = check_and_decide_path(resolved_old, OP_WRITE);
    policy_decision_t decision_new = check_and_decide_path(resolved_new, OP_WRITE);
    
    free(resolved_old);
    free(resolved_new);
    
    if (decision_old == POLICY_DENY || decision_new == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_rename(oldpath, newpath);
}

/*
 * Network operations
 */

static void extract_host_port(const struct sockaddr *addr, char *host, size_t host_len, int *port) {
    if (addr->sa_family == AF_INET) {
        struct sockaddr_in *addr_in = (struct sockaddr_in*)addr;
        inet_ntop(AF_INET, &addr_in->sin_addr, host, host_len);
        *port = ntohs(addr_in->sin_port);
    } else if (addr->sa_family == AF_INET6) {
        struct sockaddr_in6 *addr_in6 = (struct sockaddr_in6*)addr;
        inet_ntop(AF_INET6, &addr_in6->sin6_addr, host, host_len);
        *port = ntohs(addr_in6->sin6_port);
    }
}

int connect(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    pthread_once(&init_once, init_real_functions);
    
    if (!addr || !real_connect) {
        errno = EINVAL;
        return -1;
    }
    
    if (addr->sa_family == AF_INET || addr->sa_family == AF_INET6) {
        char host[256] = {0};
        int port = 0;
        extract_host_port(addr, host, sizeof(host), &port);
        
        policy_decision_t decision = policy_check_network(host, port);
        if (decision == POLICY_ASK_BROKER) {
            decision = broker_ask_network(host, port);
        }
        
        if (decision == POLICY_DENY) {
            errno = EACCES;
            return -1;
        }
    }
    
    return real_connect(sockfd, addr, addrlen);
}

int bind(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    pthread_once(&init_once, init_real_functions);
    
    if (!addr || !real_bind) {
        errno = EINVAL;
        return -1;
    }
    
    if (addr->sa_family == AF_INET || addr->sa_family == AF_INET6) {
        char host[256] = {0};
        int port = 0;
        extract_host_port(addr, host, sizeof(host), &port);
        
        policy_decision_t decision = policy_check_network(host, port);
        if (decision == POLICY_ASK_BROKER) {
            decision = broker_ask_network(host, port);
        }
        
        if (decision == POLICY_DENY) {
            errno = EACCES;
            return -1;
        }
    }
    
    return real_bind(sockfd, addr, addrlen);
}

int getaddrinfo(const char *node, const char *service, 
                const struct addrinfo *hints, struct addrinfo **res) {
    pthread_once(&init_once, init_real_functions);
    
    if (!real_getaddrinfo) {
        return EAI_FAIL;
    }
    
    if (node) {
        policy_decision_t decision = policy_check_network(node, 0);
        if (decision == POLICY_ASK_BROKER) {
            decision = broker_ask_network(node, 0);
        }
        
        if (decision == POLICY_DENY) {
            return EAI_FAIL;
        }
    }
    
    return real_getaddrinfo(node, service, hints, res);
}

/*
 * Process operations
 */

int execve(const char *path, char *const argv[], char *const envp[]) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_execve) {
        errno = EINVAL;
        return -1;
    }
    
    char *resolved = safe_realpath(path);
    policy_decision_t decision = policy_check_exec(resolved);
    if (decision == POLICY_ASK_BROKER) {
        decision = broker_ask_exec(resolved);
    }
    free(resolved);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return real_execve(path, argv, envp);
}

int posix_spawn(pid_t *pid, const char *path,
                const posix_spawn_file_actions_t *file_actions,
                const posix_spawnattr_t *attrp,
                char *const argv[], char *const envp[]) {
    pthread_once(&init_once, init_real_functions);
    
    if (!path || !real_posix_spawn) {
        return EINVAL;
    }
    
    char *resolved = safe_realpath(path);
    policy_decision_t decision = policy_check_exec(resolved);
    if (decision == POLICY_ASK_BROKER) {
        decision = broker_ask_exec(resolved);
    }
    free(resolved);
    
    if (decision == POLICY_DENY) {
        return EACCES;
    }
    
    return real_posix_spawn(pid, path, file_actions, attrp, argv, envp);
}

/*
 * Dynamic library operations
 */

void* dlopen(const char *path, int mode) {
    pthread_once(&init_once, init_real_functions);
    
    if (!real_dlopen) {
        return NULL;
    }
    
    if (path) {
        char *resolved = safe_realpath(path);
        policy_decision_t decision = policy_check_dlopen(resolved);
        if (decision == POLICY_ASK_BROKER) {
            decision = broker_ask_dlopen(resolved);
        }
        free(resolved);
        
        if (decision == POLICY_DENY) {
            return NULL;
        }
    }
    
    return real_dlopen(path, mode);
}

/*
 * Library initialization
 */

__attribute__((constructor))
static void libsandbox_init(void) {
    pthread_once(&init_once, init_real_functions);
    policy_init();
    broker_init();
}

__attribute__((destructor))
static void libsandbox_cleanup(void) {
    broker_cleanup();
    policy_cleanup();
}

#endif /* __linux__ */
