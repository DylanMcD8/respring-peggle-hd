#include "frame_pacing_policy.h"
#include <math.h>
int peggle_policy_target(PegglePacingPolicy*s,double now,int enabled,int allow,int maximum,int lp,int thermal,int active,int loading){
 int cap=120,reason=PEGGLE_NORMAL;
 if(!enabled){cap=60;reason=PEGGLE_DISABLED;}
 // The original-rate preference also applies during loading and recovery.
 else if(!allow){cap=30;reason=PEGGLE_USER30;}
 else if(!active){cap=60;reason=PEGGLE_INACTIVE;}
 else if(thermal>=3){cap=30;reason=PEGGLE_THERMAL30;}
 else if(thermal>=2){cap=60;reason=PEGGLE_THERMAL60;}
 else if(lp){cap=60;reason=PEGGLE_LOWPOWER;}
 else if(maximum<120){cap=maximum>0&&maximum<60?30:60;reason=PEGGLE_HARDWARE60;}
 else if(loading){cap=60;reason=PEGGLE_LOADING;}
 if(cap<=60){s->bad=0;s->hold_until=now+1.0;s->hold_reason=PEGGLE_RECOVERY;}
 else if(now<s->hold_until){cap=60;reason=s->hold_reason?s->hold_reason:PEGGLE_RECOVERY;}
 s->target=cap;s->reason=reason;return cap;
}
void peggle_policy_begin(PegglePacingPolicy*s){s->wait_credit=0;s->callback_active=1;}
void peggle_policy_wait(PegglePacingPolicy*s,double credit){if(s->callback_active&&isfinite(credit)&&credit>0)s->wait_credit=fmin(1.0,s->wait_credit+credit);}
void peggle_policy_resume(PegglePacingPolicy*s,double now){s->bad=0;s->composition_work=0;s->wait_credit=0;s->callback_active=0;s->hold_until=now+1.0;s->hold_reason=PEGGLE_RECOVERY;s->target=60;s->reason=PEGGLE_RECOVERY;}
void peggle_policy_sample(PegglePacingPolicy*s,double now,double elapsed,int loading){
 double composition=s->composition_work,wait=s->wait_credit;s->composition_work=0;s->wait_credit=0;s->callback_active=0;
 if(!isfinite(elapsed)||elapsed<0){s->bad=0;return;}
 wait=fmin(wait,elapsed);double work=elapsed-wait+composition;s->last_wall=elapsed;s->last_work=work;s->last_wait=wait;s->last_composition=composition;
 if(loading||s->target<=60){s->bad=0;return;}
 if(work>0.1){
  s->stalls=(s->stalls&&now>=s->last_stall&&now-s->last_stall<=10.0)?s->stalls+1:1;s->last_stall=now;
  s->hold_reason=s->stalls>=3?PEGGLE_REPEATED_STALL:PEGGLE_STALL;s->hold_until=now+(s->stalls>=3?5.0:1.0);s->target=60;s->reason=s->hold_reason;s->bad=0;return;
 }
 if(work>0.0092)s->bad++;else if(s->bad>0)s->bad--;
 if(s->bad>=48){s->hold_until=now+5.0;s->hold_reason=PEGGLE_OVERLOAD;s->target=60;s->reason=PEGGLE_OVERLOAD;s->bad=0;}
}
void peggle_policy_composition(PegglePacingPolicy*s,double elapsed,int loading){if(loading||s->target<=60){s->composition_work=0;return;}if(isfinite(elapsed)&&elapsed>=0)s->composition_work=fmin(1.0,s->composition_work+elapsed);}
// Count host callbacks over a window, rather than invert one jittered gap.
// This remains callback cadence, never a physical scan-out measurement.
int peggle_display_observe(PeggleDisplayObservation*s,double timestamp,double*rate){
 if(!isfinite(timestamp)||timestamp<=s->last||(s->last>0&&timestamp-s->last>0.25)){*s=(PeggleDisplayObservation){0};return 0;}
 if(s->last==0){s->start=s->last=timestamp;s->intervals=0;return 0;}
 s->last=timestamp;s->intervals++;double span=timestamp-s->start;
 if(span<0.5)return 0;
 *rate=s->intervals/span;s->start=timestamp;s->intervals=0;return 1;
}
