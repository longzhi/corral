/*
 * Test program for libsandbox
 * 
 * This program attempts various operations that should be intercepted
 * by the sandbox library.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <errno.h>

void test_file_read(const char *path) {
    printf("Testing file read: %s\n", path);
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        printf("  BLOCKED: %s\n", strerror(errno));
    } else {
        printf("  ALLOWED\n");
        close(fd);
    }
}

void test_file_write(const char *path) {
    printf("Testing file write: %s\n", path);
    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        printf("  BLOCKED: %s\n", strerror(errno));
    } else {
        printf("  ALLOWED\n");
        write(fd, "test\n", 5);
        close(fd);
        unlink(path);
    }
}

void test_network_connect(const char *host, int port) {
    printf("Testing network connect: %s:%d\n", host, port);
    
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    inet_pton(AF_INET, host, &addr.sin_addr);
    
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    if (sock < 0) {
        printf("  ERROR: socket() failed\n");
        return;
    }
    
    if (connect(sock, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        if (errno == EACCES) {
            printf("  BLOCKED by sandbox\n");
        } else {
            printf("  FAILED: %s\n", strerror(errno));
        }
    } else {
        printf("  ALLOWED (connected)\n");
    }
    
    close(sock);
}

void test_dns_lookup(const char *hostname) {
    printf("Testing DNS lookup: %s\n", hostname);
    
    struct addrinfo hints, *result;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;
    
    int ret = getaddrinfo(hostname, NULL, &hints, &result);
    if (ret != 0) {
        if (ret == EAI_FAIL) {
            printf("  BLOCKED by sandbox\n");
        } else {
            printf("  FAILED: %s\n", gai_strerror(ret));
        }
    } else {
        printf("  ALLOWED\n");
        freeaddrinfo(result);
    }
}

void test_exec(const char *program) {
    printf("Testing exec: %s\n", program);
    
    pid_t pid = fork();
    if (pid == 0) {
        /* Child process */
        char *argv[] = {(char*)program, NULL};
        char *envp[] = {NULL};
        execve(program, argv, envp);
        
        /* If execve returns, it failed */
        if (errno == EACCES) {
            printf("  BLOCKED by sandbox\n");
        } else {
            printf("  FAILED: %s\n", strerror(errno));
        }
        exit(1);
    } else if (pid > 0) {
        /* Parent process */
        int status;
        waitpid(pid, &status, 0);
    } else {
        printf("  ERROR: fork() failed\n");
    }
}

int main(int argc, char *argv[]) {
    printf("=== libsandbox Test Program ===\n\n");
    
    /* File tests */
    printf("--- File Access Tests ---\n");
    test_file_read("/etc/passwd");
    test_file_read("/tmp/test.txt");
    test_file_write("/tmp/sandbox_test.txt");
    test_file_write("/etc/test.txt");
    printf("\n");
    
    /* Network tests */
    printf("--- Network Tests ---\n");
    test_network_connect("127.0.0.1", 80);
    test_network_connect("8.8.8.8", 53);
    test_dns_lookup("google.com");
    test_dns_lookup("example.com");
    printf("\n");
    
    /* Exec tests */
    printf("--- Exec Tests ---\n");
    test_exec("/bin/ls");
    test_exec("/usr/bin/whoami");
    printf("\n");
    
    printf("=== Test Complete ===\n");
    return 0;
}
