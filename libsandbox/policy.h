#ifndef SANDBOX_POLICY_H
#define SANDBOX_POLICY_H

#include <stdbool.h>
#include <stddef.h>

/* Policy decision types */
typedef enum {
    POLICY_ALLOW,
    POLICY_DENY,
    POLICY_ASK_BROKER
} policy_decision_t;

/* Operation types */
typedef enum {
    OP_READ,
    OP_WRITE,
    OP_EXEC
} operation_t;

/* Initialize policy from environment variables */
bool policy_init(void);

/* Cleanup policy resources */
void policy_cleanup(void);

/* Check if a file path operation is allowed */
policy_decision_t policy_check_path(const char *path, operation_t op);

/* Check if a network connection is allowed */
policy_decision_t policy_check_network(const char *host, int port);

/* Check if an executable can be launched */
policy_decision_t policy_check_exec(const char *path);

/* Check if a library can be loaded */
policy_decision_t policy_check_dlopen(const char *path);

#endif /* SANDBOX_POLICY_H */
