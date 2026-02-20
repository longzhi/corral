# Test Makefile

CC ?= cc
CFLAGS = -Wall -Wextra -g

test_sandbox: test_sandbox.c
	$(CC) $(CFLAGS) -o $@ $^

.PHONY: clean
clean:
	rm -f test_sandbox
