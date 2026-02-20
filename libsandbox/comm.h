#ifndef SANDBOX_COMM_H
#define SANDBOX_COMM_H

#include <stdbool.h>
#include "policy.h"

/* Initialize broker connection */
bool broker_init(void);

/* Cleanup broker connection */
void broker_cleanup(void);

/* Ask broker for policy decision on a file path */
policy_decision_t broker_ask_path(const char *path, operation_t op);

/* Ask broker for policy decision on network connection */
policy_decision_t broker_ask_network(const char *host, int port);

/* Ask broker for policy decision on exec */
policy_decision_t broker_ask_exec(const char *path);

/* Ask broker for policy decision on dlopen */
policy_decision_t broker_ask_dlopen(const char *path);

#endif /* SANDBOX_COMM_H */
