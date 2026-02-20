#ifdef __APPLE__

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

/* DYLD interpose macro */
#define DYLD_INTERPOSE(_replacement, _original) \
    __attribute__((used, section("__DATA,__interpose"))) \
    static struct { void* r; void* o; } _##_original##_interpose = \
    { (void*)_replacement, (void*)_original };

/* Original function pointers (for functions we can't directly reference) */
static int (*original_open)(const char*, int, ...) = NULL;

/* Helper: Check path and decide */
static inline policy_decision_t check_and_decide_path(const char *path, operation_t op) {
    policy_decision_t decision = policy_check_path(path, op);
    if (decision == POLICY_ASK_BROKER) {
        decision = broker_ask_path(path, op);
    }
    return decision;
}

/* Helper: Realpath wrapper that doesn't fail on non-existent paths */
static char* safe_realpath(const char *path) {
    char *resolved = realpath(path, NULL);
    if (!resolved) {
        /* If realpath fails, use the path as-is */
        return strdup(path);
    }
    return resolved;
}

/*
 * File operations
 */

/* open() - intercept file opens */
int my_open(const char *path, int flags, ...) {
    if (!original_open) {
        original_open = (int (*)(const char*, int, ...))dlsym(RTLD_NEXT, "open");
    }
    
    if (!path) {
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
    
    /* Call original open with varargs */
    mode_t mode = 0;
    if (flags & O_CREAT) {
        va_list args;
        va_start(args, flags);
        mode = va_arg(args, int);
        va_end(args);
        return original_open(path, flags, mode);
    }
    
    return original_open(path, flags);
}
DYLD_INTERPOSE(my_open, open)

/* access() - check file accessibility */
int my_access(const char *path, int mode) {
    if (!path) {
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
    
    return access(path, mode);
}
DYLD_INTERPOSE(my_access, access)

/* stat() - get file status */
int my_stat(const char *path, struct stat *buf) {
    if (!path) {
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
    
    return stat(path, buf);
}
DYLD_INTERPOSE(my_stat, stat)

/* lstat() - get file status (don't follow symlinks) */
int my_lstat(const char *path, struct stat *buf) {
    if (!path) {
        errno = EINVAL;
        return -1;
    }
    
    /* For lstat, don't resolve the path */
    policy_decision_t decision = check_and_decide_path(path, OP_READ);
    
    if (decision == POLICY_DENY) {
        errno = EACCES;
        return -1;
    }
    
    return lstat(path, buf);
}
DYLD_INTERPOSE(my_lstat, lstat)

/* unlink() - delete file */
int my_unlink(const char *path) {
    if (!path) {
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
    
    return unlink(path);
}
DYLD_INTERPOSE(my_unlink, unlink)

/* rename() - rename file */
int my_rename(const char *oldpath, const char *newpath) {
    if (!oldpath || !newpath) {
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
    
    return rename(oldpath, newpath);
}
DYLD_INTERPOSE(my_rename, rename)

/*
 * Network operations
 */

/* Helper: Extract host and port from sockaddr */
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

/* connect() - initiate connection */
int my_connect(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    if (!addr) {
        errno = EINVAL;
        return -1;
    }
    
    /* Only check for INET/INET6 sockets */
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
    
    return connect(sockfd, addr, addrlen);
}
DYLD_INTERPOSE(my_connect, connect)

/* bind() - bind socket to address */
int my_bind(int sockfd, const struct sockaddr *addr, socklen_t addrlen) {
    if (!addr) {
        errno = EINVAL;
        return -1;
    }
    
    /* Only check for INET/INET6 sockets */
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
    
    return bind(sockfd, addr, addrlen);
}
DYLD_INTERPOSE(my_bind, bind)

/* getaddrinfo() - network address and service translation */
int my_getaddrinfo(const char *node, const char *service, 
                   const struct addrinfo *hints, struct addrinfo **res) {
    if (node) {
        /* Check if DNS resolution is allowed for this host */
        policy_decision_t decision = policy_check_network(node, 0);
        if (decision == POLICY_ASK_BROKER) {
            decision = broker_ask_network(node, 0);
        }
        
        if (decision == POLICY_DENY) {
            return EAI_FAIL;
        }
    }
    
    return getaddrinfo(node, service, hints, res);
}
DYLD_INTERPOSE(my_getaddrinfo, getaddrinfo)

/*
 * Process/exec operations
 */

/* execve() - execute program */
int my_execve(const char *path, char *const argv[], char *const envp[]) {
    if (!path) {
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
    
    return execve(path, argv, envp);
}
DYLD_INTERPOSE(my_execve, execve)

/* posix_spawn() - spawn a process */
int my_posix_spawn(pid_t *pid, const char *path,
                   const posix_spawn_file_actions_t *file_actions,
                   const posix_spawnattr_t *attrp,
                   char *const argv[], char *const envp[]) {
    if (!path) {
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
    
    return posix_spawn(pid, path, file_actions, attrp, argv, envp);
}
DYLD_INTERPOSE(my_posix_spawn, posix_spawn)

/*
 * Dynamic library operations
 */

/* dlopen() - load dynamic library */
void* my_dlopen(const char *path, int mode) {
    if (path) {  /* NULL path means RTLD_DEFAULT */
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
    
    return dlopen(path, mode);
}
DYLD_INTERPOSE(my_dlopen, dlopen)

/*
 * Library initialization/cleanup
 */

__attribute__((constructor))
static void libsandbox_init(void) {
    /* Initialize policy from environment */
    policy_init();
    
    /* Try to initialize broker connection (optional) */
    broker_init();
}

__attribute__((destructor))
static void libsandbox_cleanup(void) {
    broker_cleanup();
    policy_cleanup();
}

#endif /* __APPLE__ */
