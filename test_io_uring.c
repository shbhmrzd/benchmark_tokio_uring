// C code to test io_uring support on a system
#include <linux/io_uring.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <stdio.h>
#include <errno.h>
#include <string.h>

int main() {
    struct io_uring_params params;
    memset(&params, 0, sizeof(params)); // Initialize params to zero

    // Call io_uring_setup syscall
    int ring_fd = syscall(SYS_io_uring_setup, 8, &params);
    if (ring_fd < 0) {
        printf("io_uring_setup failed: %s\n", strerror(errno));
        return 1;
    }

    printf("io_uring is supported!\n");
    close(ring_fd);
    return 0;
}

//[prod] sraizada@rccp109-5b:~ $ gcc -o io_uring_test io_uring_test.c
//[prod] sraizada@rccp109-5b:~ $ ./io_uring_test