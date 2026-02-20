# Makefile for libsandbox interposition library

CC ?= cc
CFLAGS = -Wall -Wextra -fPIC -O2 -g
LDFLAGS = -shared

# Source files
COMMON_SRCS = policy.c comm.c cJSON.c
MACOS_SRCS = interpose_macos.c
LINUX_SRCS = interpose_linux.c

# Output files
MACOS_LIB = libsandbox.dylib
LINUX_LIB = libsandbox.so

# Platform detection
UNAME_S := $(shell uname -s)

.PHONY: all clean macos linux test

# Default target based on platform
all:
ifeq ($(UNAME_S),Darwin)
	@$(MAKE) macos
else ifeq ($(UNAME_S),Linux)
	@$(MAKE) linux
else
	@echo "Unsupported platform: $(UNAME_S)"
	@exit 1
endif

# macOS target
macos: $(MACOS_LIB)

$(MACOS_LIB): $(COMMON_SRCS) $(MACOS_SRCS)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $^ -lpthread
	@echo "Built $@ successfully"

# Linux target
linux: $(LINUX_LIB)

$(LINUX_LIB): $(COMMON_SRCS) $(LINUX_SRCS)
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $^ -lpthread -ldl
	@echo "Built $@ successfully"

# Test build
test: all
	@echo "Building test program..."
	@cd tests && $(MAKE)

# Clean
clean:
	rm -f $(MACOS_LIB) $(LINUX_LIB) *.o
	@cd tests 2>/dev/null && $(MAKE) clean || true

# Install (optional)
install: all
ifeq ($(UNAME_S),Darwin)
	install -m 755 $(MACOS_LIB) /usr/local/lib/
else ifeq ($(UNAME_S),Linux)
	install -m 755 $(LINUX_LIB) /usr/local/lib/
endif

.PHONY: help
help:
	@echo "libsandbox Makefile"
	@echo ""
	@echo "Targets:"
	@echo "  all      - Build for current platform (default)"
	@echo "  macos    - Build libsandbox.dylib for macOS"
	@echo "  linux    - Build libsandbox.so for Linux"
	@echo "  test     - Build and run tests"
	@echo "  clean    - Remove build artifacts"
	@echo "  install  - Install library to /usr/local/lib"
	@echo "  help     - Show this help"
