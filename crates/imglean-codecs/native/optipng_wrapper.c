#include <stdarg.h>
#include <stdlib.h>
#include <string.h>

#include "optipng.h"

static void quiet_printf(const char *format, ...)
{
    (void)format;
}

static void quiet_control(int control_code)
{
    (void)control_code;
}

static void quiet_progress(unsigned long current, unsigned long total)
{
    (void)current;
    (void)total;
}

static void abort_on_panic(const char *message)
{
    (void)message;
    abort();
}

int imglean_optipng_optimize(const char *input, const char *output,
                             int strip_metadata)
{
    struct opng_options options;
    struct opng_ui ui;
    int result;

    memset(&options, 0, sizeof(options));
    options.optim_level = 2;
    options.interlace = -1;
    options.quiet = 1;
    options.out_name = output;
    options.strip_all = strip_metadata != 0;

    ui.printf_fn = quiet_printf;
    ui.print_cntrl_fn = quiet_control;
    ui.progress_fn = quiet_progress;
    ui.panic_fn = abort_on_panic;

    if (opng_initialize(&options, &ui) != 0)
        return -1;
    result = opng_optimize(input);
    if (opng_finalize() != 0)
        return -1;
    return result;
}
