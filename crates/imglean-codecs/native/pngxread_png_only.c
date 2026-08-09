/* PNG-only adaptation of OptiPNG 7.9.1 src/pngxtern/pngxread.c. */
/* Distributed under the same license and warranty terms as libpng. */

#include "pngxtern.h"
#include <stdio.h>
#include <string.h>

int PNGAPI
pngx_read_image(png_structp png_ptr, png_infop info_ptr,
                png_const_charpp fmt_name_ptr,
                png_const_charpp fmt_long_name_ptr)
{
    static const png_byte png_signature[8] =
        {137, 80, 78, 71, 13, 10, 26, 10};
    static const char format_name[] = "PNG";
    static const char format_long_name[] = "Portable Network Graphics";
    png_byte signature[8];
    FILE *stream = (FILE *)png_get_io_ptr(png_ptr);

    if (fread(signature, 1, sizeof(signature), stream) != sizeof(signature))
        return 0;
    if (memcmp(signature, png_signature, sizeof(signature)) != 0)
        return 0;
    if (fseek(stream, 0, SEEK_SET) != 0)
        png_error(png_ptr, "Can't rewind PNG input stream");
    if (fmt_name_ptr != NULL)
        *fmt_name_ptr = format_name;
    if (fmt_long_name_ptr != NULL)
        *fmt_long_name_ptr = format_long_name;
    png_read_png(png_ptr, info_ptr, 0, NULL);
    if (getc(stream) != EOF)
        png_error(png_ptr, "Extraneous data found after IEND");
    return 1;
}
