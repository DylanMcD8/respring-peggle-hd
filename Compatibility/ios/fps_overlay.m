#import <UIKit/UIKit.h>
#import <QuartzCore/QuartzCore.h>
#include <os/lock.h>
#include <stdatomic.h>
#include <time.h>
#include "fps_counter.h"

static _Thread_local unsigned gamePresentDepth;
static atomic_bool overlayEnabled;
static os_unfair_lock counterLock=OS_UNFAIR_LOCK_INIT;
static PeggleFPSCounter counter;
extern void peggle_ios_fps_diagnostic(int kind,double fps,unsigned long long count,int warming,int overflow,double x,double y,double width,double height,int interactive,int contained);
static uint64_t nowNS(void){struct timespec t={0};clock_gettime(CLOCK_MONOTONIC,&t);return(uint64_t)t.tv_sec*UINT64_C(1000000000)+(uint64_t)t.tv_nsec;}
void peggle_fps_begin_game_present(void){gamePresentDepth++;}
void peggle_fps_end_game_present(void){if(gamePresentDepth)gamePresentDepth--;}

/* Numeric UI only. Warming displays no number; the unchanged counter and
 * diagnostics retain warming/overflow flags and successful-present semantics. */
static NSString *peggle_numeric_fps_text(PeggleFPSReading reading){
 return reading.warming?@"":[NSString stringWithFormat:@"%.0f",reading.fps];
}
static NSString *peggle_numeric_fps_hint(PeggleFPSReading reading){
 if(reading.warming)return @"Warming up. Successful game buffer presentations per second over half a second.";
 if(reading.overflow)return @"At least this many successful game buffer presentations per second over half a second.";
 return @"Successful game buffer presentations per second over half a second. Hide or show in this app’s iPad Settings.";
}

@interface PeggleFPSOverlay:NSObject
@property(nonatomic,strong)UILabel *label;
@property(nonatomic,weak)UIView *root;
@property(nonatomic,strong)NSTimer *timer;
@property(nonatomic,strong)NSArray *observers;
@property(nonatomic,assign)uint64_t lastDiagnosticNS;
- (void)mountForPresentedView:(UIView *)view;
- (void)refreshPreferencesAndActivity;
- (void)update;
@end
static PeggleFPSOverlay *overlay;
@implementation PeggleFPSOverlay
- (instancetype)init{
 if((self=[super init])){
  __weak PeggleFPSOverlay *weakSelf=self;NSMutableArray *tokens=[NSMutableArray new];
  for(NSNotificationName name in @[UIApplicationDidBecomeActiveNotification,UIApplicationDidEnterBackgroundNotification]){
   id token=[NSNotificationCenter.defaultCenter addObserverForName:name object:nil queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){[weakSelf refreshPreferencesAndActivity];}];[tokens addObject:token];
  }self.observers=tokens;[self refreshPreferencesAndActivity];
 }return self;
}
- (void)refreshPreferencesAndActivity{
 BOOL enabled=[NSProcessInfo.processInfo.environment[@"PEGGLE_DEBUG_FPS"] isEqualToString:@"1"]&&UIApplication.sharedApplication.applicationState==UIApplicationStateActive;
 BOOL old=atomic_exchange_explicit(&overlayEnabled,enabled,memory_order_relaxed);
 if(old!=enabled){os_unfair_lock_lock(&counterLock);peggle_fps_counter_reset(&counter);os_unfair_lock_unlock(&counterLock);}
 self.label.hidden=!enabled;
 if(old!=enabled){self.lastDiagnosticNS=0;peggle_ios_fps_diagnostic(0,0,enabled,0,0,0,0,0,0,0,0);}
 if(enabled&&!self.timer){__weak PeggleFPSOverlay *weakSelf=self;self.timer=[NSTimer timerWithTimeInterval:0.25 repeats:YES block:^(NSTimer *timer){[weakSelf update];}];[[NSRunLoop mainRunLoop]addTimer:self.timer forMode:NSRunLoopCommonModes];self.timer.tolerance=0.025;}
 if(!enabled&&self.timer){[self.timer invalidate];self.timer=nil;}
}
- (void)mountForPresentedView:(UIView *)view{
 UIView *root=view.window.rootViewController.view;if(!root||self.root==root)return;
 [self.label removeFromSuperview];self.root=root;
 if(!self.label){UILabel *label=[UILabel new];label.translatesAutoresizingMaskIntoConstraints=NO;label.userInteractionEnabled=NO;label.textAlignment=NSTextAlignmentCenter;label.font=[UIFont monospacedDigitSystemFontOfSize:36 weight:UIFontWeightSemibold];label.textColor=UIColor.whiteColor;label.backgroundColor=[UIColor colorWithWhite:0 alpha:0.62];label.layer.cornerRadius=5;label.clipsToBounds=YES;label.adjustsFontSizeToFitWidth=NO;label.minimumScaleFactor=1.0;label.numberOfLines=1;label.text=@"";label.isAccessibilityElement=YES;label.accessibilityHint=@"Successful game buffer presentations per second over half a second. Hide or show in this app’s iPad Settings.";self.label=label;}
 [root addSubview:self.label];UILayoutGuide *safe=root.safeAreaLayoutGuide;NSLayoutConstraint *preferred=[self.label.widthAnchor constraintEqualToConstant:128];preferred.priority=UILayoutPriorityDefaultHigh;
 [NSLayoutConstraint activateConstraints:@[[self.label.leadingAnchor constraintEqualToAnchor:safe.leadingAnchor constant:8],[self.label.topAnchor constraintEqualToAnchor:safe.topAnchor constant:8],[self.label.widthAnchor constraintLessThanOrEqualToAnchor:safe.widthAnchor constant:-16],[self.label.heightAnchor constraintEqualToConstant:72],preferred]];
 self.label.hidden=!atomic_load_explicit(&overlayEnabled,memory_order_relaxed);
}
- (void)update{
 if(!atomic_load_explicit(&overlayEnabled,memory_order_relaxed))return;
 os_unfair_lock_lock(&counterLock);PeggleFPSReading r=peggle_fps_counter_read(&counter,nowNS());os_unfair_lock_unlock(&counterLock);
 NSString *text=peggle_numeric_fps_text(r);
 if(![self.label.text isEqualToString:text]){self.label.text=text;self.label.accessibilityLabel=text;}
 self.label.accessibilityHint=peggle_numeric_fps_hint(r);
 uint64_t t=nowNS();if(self.label&&self.root&&(self.lastDiagnosticNS==0||t-self.lastDiagnosticNS>=UINT64_C(5000000000))){self.lastDiagnosticNS=t;CGRect frame=self.label.frame;BOOL contained=CGRectContainsRect(self.root.safeAreaLayoutGuide.layoutFrame,frame);peggle_ios_fps_diagnostic(1,r.fps,r.successful_presentations,r.warming,r.overflow,frame.origin.x,frame.origin.y,frame.size.width,frame.size.height,self.label.userInteractionEnabled,contained);}
}
@end
void peggle_install_fps_overlay(void){NSCAssert(NSThread.isMainThread,@"FPS overlay setup must use the UIKit thread");static dispatch_once_t once;dispatch_once(&once,^{overlay=[PeggleFPSOverlay new];});}
/* Invoked only after SDL's actual EAGL presentRenderbuffer: result. A game tag
 * excludes bootstrap/non-game swaps; failed presents never enter the counter.
 * This measures accepted native submissions, not physical display scanout. */
void peggle_fps_present_result(void *nativeView,int success){
 if(!atomic_load_explicit(&overlayEnabled,memory_order_relaxed))return;
 uint64_t t=nowNS();os_unfair_lock_lock(&counterLock);peggle_fps_counter_record(&counter,t,gamePresentDepth!=0,success!=0);os_unfair_lock_unlock(&counterLock);
 if(gamePresentDepth&&success&&NSThread.isMainThread)[overlay mountForPresentedView:(__bridge UIView *)nativeView];
}
