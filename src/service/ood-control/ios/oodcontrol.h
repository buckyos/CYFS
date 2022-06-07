#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "ios_logger.h"

typedef struct Result {
  uintptr_t result_num;
  char **result_value;
} Result;

void free_result(struct Result *result);

void init(const char *cbase_path, const char *cloglevel, LogCallback log);

void start(void);
void wait_bind(void);

unsigned char is_bind(void);

const struct Result *get_address_list(void);
