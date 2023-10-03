#ifndef electorium_fuzzable_H
#define electorium_fuzzable_H

// This file is generated from src/rffi.rs using cbindgen

#include "stdint.h"

typedef struct Fuzz Fuzz;

const Fuzz *electorium_fuzz_new(bool verbose);

void electorium_fuzz_destroy(const Fuzz *f);

int16_t electorium_fuzz_run(const Fuzz *f, const uint8_t *buf, uintptr_t len);

#endif /* electorium_fuzzable_H */
