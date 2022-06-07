#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "ios_logger.h"
void startPerfClient(const char *cowner_id,
                     const char *cdec_id,
                     const char *cclient_addr,
                     const char *cbase_path,
                     const char *cnon_addr,
                     const char *cws_addr,
                     int cbdt_port,
                     const char *cloglevel,
                     const char *cwifi_addr,
                     LogCallback log);

void statPerf(const char *id,
              const char *key,
              int bytes,
              int error_code,
              const char *name,
              const char *value);
