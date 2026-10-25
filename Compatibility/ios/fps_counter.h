/* C28 successful game-presentation counter. No UIKit or game data dependency. */
#ifndef PEGGLE_FPS_COUNTER_H
#define PEGGLE_FPS_COUNTER_H
#include <stdint.h>
#include <stdbool.h>
#define PEGGLE_FPS_WINDOW_NS UINT64_C(500000000)
#define PEGGLE_FPS_CAPACITY 1024u
/* Fixed8KiB ring; no allocation or display-callback counting. */
typedef struct { uint64_t times[PEGGLE_FPS_CAPACITY], first_ns, last_ns, discarded_ns; unsigned head,count; bool started,overflow; } PeggleFPSCounter;
typedef struct { double fps; unsigned successful_presentations; bool warming,overflow; } PeggleFPSReading;
void peggle_fps_counter_reset(PeggleFPSCounter *counter);
void peggle_fps_counter_record(PeggleFPSCounter *counter,uint64_t now_ns,bool is_game,bool success);
PeggleFPSReading peggle_fps_counter_read(PeggleFPSCounter *counter,uint64_t now_ns);
#endif
