#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "ios_logger.h"

void start(const char *cbase_path,
           const char *cnon_addr,
           const char *cws_addr,
           int cbdt_port,
           const char *cloglevel,
           const char *cwifi_addr,
           LogCallback log);

void restartInterface();

void resetNetwork(const char *cwifi_addr);
