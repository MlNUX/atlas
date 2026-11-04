// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later
// C ABI of spikes/terminal/core (variant B of the terminal spike, P0.5).
#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define ATS_REDRAW 1
#define ATS_TITLE 2
#define ATS_BELL 3
#define ATS_EXITED 4

typedef struct AtsTerminal AtsTerminal;
typedef void (*AtsEventFn)(void *ctx, uint32_t kind, int32_t code);

/// argc == 0 starts the default shell. Returns NULL on failure.
AtsTerminal *ats_spawn(const char *const *argv, size_t argc, const char *cwd, uint16_t cols, uint16_t lines,
                       uint16_t cell_width, uint16_t cell_height, AtsEventFn callback, void *ctx);
void ats_write(AtsTerminal *t, const uint8_t *bytes, size_t len);
void ats_resize(AtsTerminal *t, uint16_t cols, uint16_t lines, uint16_t cell_width, uint16_t cell_height);
void ats_scroll(AtsTerminal *t, int32_t lines);
/// JSON of the visible screen; free with ats_string_free.
char *ats_frame_json(AtsTerminal *t, bool focused);
/// Screen text plus `history` lines of scrollback; free with ats_string_free.
char *ats_text(AtsTerminal *t, size_t history);
void ats_string_free(char *s);
void ats_free(AtsTerminal *t);
