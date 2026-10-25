/* Local Peggle fork: fixed completed-frame output; guest rasters unchanged. */
#ifndef PEGGLE_OUTPUT_POLICY_H
#define PEGGLE_OUTPUT_POLICY_H
#include <math.h>
/* Aspect preserving, orientation independent. A full 4:3 window gets exactly
 * 2048x1536 (landscape) / 1536x2048 (portrait). Non-4:3 windows fit within that
 * box and the existing viewport supplies bars. No physical-panel inference. */
static inline double peggle_output_scale(double width,double height,double fallback) {
    if (!isfinite(width)||!isfinite(height)||width<=0||height<=0) return fallback;
    double long_edge=fmax(width,height),short_edge=fmin(width,height);
    return fmin(2048.0/long_edge,1536.0/short_edge);
}
#endif
