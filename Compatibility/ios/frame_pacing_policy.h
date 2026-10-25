#pragma once
#include <stdint.h>
enum{PEGGLE_NORMAL=1,PEGGLE_DISABLED,PEGGLE_USER30,PEGGLE_HARDWARE60,PEGGLE_LOWPOWER,PEGGLE_THERMAL60,PEGGLE_THERMAL30,PEGGLE_INACTIVE,PEGGLE_LOADING,PEGGLE_RECOVERY,PEGGLE_OVERLOAD,PEGGLE_STALL,PEGGLE_REPEATED_STALL};
typedef struct {int target,bad;double hold_until,composition_work,wait_credit;int reason,hold_reason;uint32_t stalls,callback_active;double last_stall,last_wall,last_work,last_wait,last_composition;} PegglePacingPolicy;
int peggle_policy_target(PegglePacingPolicy*,double,int,int,int,int,int,int,int);
void peggle_policy_sample(PegglePacingPolicy*,double,double,int);
void peggle_policy_composition(PegglePacingPolicy*,double,int);
void peggle_policy_begin(PegglePacingPolicy*);
void peggle_policy_wait(PegglePacingPolicy*,double);
void peggle_policy_resume(PegglePacingPolicy*,double);
typedef struct{double start,last;unsigned intervals;} PeggleDisplayObservation;
int peggle_display_observe(PeggleDisplayObservation*,double,double*);
