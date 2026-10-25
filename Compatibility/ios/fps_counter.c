#include "fps_counter.h"
#include <string.h>
void peggle_fps_counter_reset(PeggleFPSCounter *c){memset(c,0,sizeof(*c));}
static void expire(PeggleFPSCounter *c,uint64_t now){
 while(c->count){uint64_t t=c->times[c->head];if(now<t||now-t<PEGGLE_FPS_WINDOW_NS)break;c->head=(c->head+1)%PEGGLE_FPS_CAPACITY;c->count--;}
 if(c->overflow&&now>=c->discarded_ns&&now-c->discarded_ns>=PEGGLE_FPS_WINDOW_NS)c->overflow=false;
}
void peggle_fps_counter_record(PeggleFPSCounter *c,uint64_t now,bool game,bool success){
 if(!game||!success)return;
 if(c->started&&now<c->last_ns)peggle_fps_counter_reset(c);
 if(!c->started){c->started=true;c->first_ns=now;}c->last_ns=now;expire(c,now);
 if(c->count==PEGGLE_FPS_CAPACITY){c->discarded_ns=c->times[c->head];c->head=(c->head+1)%PEGGLE_FPS_CAPACITY;c->count--;c->overflow=true;}
 c->times[(c->head+c->count)%PEGGLE_FPS_CAPACITY]=now;c->count++;
}
PeggleFPSReading peggle_fps_counter_read(PeggleFPSCounter *c,uint64_t now){
 if(c->started&&now<c->last_ns)peggle_fps_counter_reset(c);
 expire(c,now);PeggleFPSReading r={0};r.successful_presentations=c->count;r.fps=(double)c->count*1e9/(double)PEGGLE_FPS_WINDOW_NS;r.warming=!c->started||now<c->first_ns||now-c->first_ns<PEGGLE_FPS_WINDOW_NS;r.overflow=c->overflow;return r;
}
