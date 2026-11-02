#ifndef COMMANDS_H
#define COMMANDS_H

#include <stdint.h>

/* Set by the REBOOT_ROM command; main() reboots once the reply has been collected by the host. */
extern __data uint8_t reboot_requested;

#endif /* COMMANDS_H */
