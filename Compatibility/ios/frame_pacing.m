#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#import <os/lock.h>
#include <stdatomic.h>
#include <math.h>
#include "frame_pacing_policy.h"
static atomic_int enabled, allowed, maximum, lowPower, thermal, active, requested;
static _Atomic double observed;
static os_unfair_lock policyLock=OS_UNFAIR_LOCK_INIT;
static PegglePacingPolicy policy;
static PeggleDisplayObservation observation;
void peggle_pacing_configure(int value) {os_unfair_lock_lock(&policyLock);policy=(PegglePacingPolicy){0};atomic_store(&enabled,value);os_unfair_lock_unlock(&policyLock);}
int peggle_pacing_target(int loading) {
 os_unfair_lock_lock(&policyLock);
 int target=peggle_policy_target(&policy,CACurrentMediaTime(),atomic_load(&enabled),atomic_load(&allowed),atomic_load(&maximum),atomic_load(&lowPower),atomic_load(&thermal),atomic_load(&active),loading);
 atomic_store(&requested,target);os_unfair_lock_unlock(&policyLock);return target;
}
void peggle_pacing_sample(double seconds,int loading) {
 os_unfair_lock_lock(&policyLock);peggle_policy_sample(&policy,CACurrentMediaTime(),seconds,loading);os_unfair_lock_unlock(&policyLock);
}
void peggle_pacing_composition(double seconds,int loading){os_unfair_lock_lock(&policyLock);peggle_policy_composition(&policy,seconds,loading);os_unfair_lock_unlock(&policyLock);}
void peggle_pacing_begin(void){os_unfair_lock_lock(&policyLock);peggle_policy_begin(&policy);os_unfair_lock_unlock(&policyLock);}
void peggle_pacing_wait(double credit){os_unfair_lock_lock(&policyLock);peggle_policy_wait(&policy,credit);os_unfair_lock_unlock(&policyLock);}
typedef struct{int32_t enabled,allowed,maximum,low_power,thermal,active,loading,target,reason,hold_reason,bad;double hold_remaining,game_wall,work,wait,composition;} PegglePacingStatus;
void peggle_pacing_status(PegglePacingStatus*out){
 os_unfair_lock_lock(&policyLock);*out=(PegglePacingStatus){atomic_load(&enabled),atomic_load(&allowed),atomic_load(&maximum),atomic_load(&lowPower),atomic_load(&thermal),atomic_load(&active),0,policy.target,policy.reason,policy.hold_reason,policy.bad,fmax(0,policy.hold_until-CACurrentMediaTime()),policy.last_wall,policy.last_work,policy.last_wait,policy.last_composition};os_unfair_lock_unlock(&policyLock);
}
double peggle_pacing_observed(void) {return atomic_load(&observed);}
@interface PegglePacingMonitor:NSObject
@property(nonatomic,strong) CADisplayLink *link;
@property(nonatomic) NSInteger rate;
@property(nonatomic) double refreshed;
@end
@implementation PegglePacingMonitor
- (void)refresh {
 NSProcessInfo *p=NSProcessInfo.processInfo;
 UIWindowScene *scene=nil;
 for(UIScene *s in UIApplication.sharedApplication.connectedScenes) if([s isKindOfClass:UIWindowScene.class] && s.activationState==UISceneActivationStateForegroundActive) {scene=(UIWindowScene*)s;break;}
 int wasActive=atomic_exchange(&active,scene!=nil);if(!wasActive&&scene){os_unfair_lock_lock(&policyLock);peggle_policy_resume(&policy,CACurrentMediaTime());os_unfair_lock_unlock(&policyLock);}
 atomic_store(&maximum,(int)scene.screen.maximumFramesPerSecond);
 atomic_store(&thermal,(int)p.thermalState);atomic_store(&lowPower,p.lowPowerModeEnabled);
 atomic_store(&allowed,[NSUserDefaults.standardUserDefaults boolForKey:@"PeggleAllowHighRefresh"]);
 self.refreshed=CACurrentMediaTime();
 self.link.paused=scene==nil;
 if(!scene) {observation=(PeggleDisplayObservation){0};atomic_store(&observed,0);}
}
- (void)tick:(CADisplayLink*)link {
 // This monitor observes host display callbacks. Guest timer and accepted EAGL
 // presentation counts remain separate metrics; no physical scan-out claim.
 if(CACurrentMediaTime()-self.refreshed>=1.0)[self refresh];
 double rate=0;if(peggle_display_observe(&observation,link.timestamp,&rate))atomic_store(&observed,rate);
 NSInteger cap=atomic_load(&requested);if(cap<=0) cap=60;
 if(self.rate!=cap) {self.rate=cap;link.preferredFrameRateRange=CAFrameRateRangeMake(MIN(30,cap),cap,cap);}
}
@end
void peggle_install_frame_pacing(void) {
 [NSUserDefaults.standardUserDefaults registerDefaults:@{@"PeggleAllowHighRefresh":@YES}];
 static PegglePacingMonitor *monitor;
 monitor=[PegglePacingMonitor new];monitor.link=[CADisplayLink displayLinkWithTarget:monitor selector:@selector(tick:)];
 [monitor.link addToRunLoop:NSRunLoop.mainRunLoop forMode:NSRunLoopCommonModes];
 NSNotificationCenter *c=NSNotificationCenter.defaultCenter;
 for(NSString *name in @[NSProcessInfoThermalStateDidChangeNotification,NSProcessInfoPowerStateDidChangeNotification,NSUserDefaultsDidChangeNotification,UISceneDidActivateNotification,UISceneWillDeactivateNotification,UIApplicationDidBecomeActiveNotification,UIApplicationDidEnterBackgroundNotification])
 [c addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *n){[monitor refresh];if([n.name isEqualToString:UISceneWillDeactivateNotification]||[n.name isEqualToString:UIApplicationDidEnterBackgroundNotification]){atomic_store(&active,0);monitor.link.paused=YES;observation=(PeggleDisplayObservation){0};atomic_store(&observed,0);}}];
 [monitor refresh];
}
