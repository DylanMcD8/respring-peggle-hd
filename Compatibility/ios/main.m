#include <SDL.h>
#import "auto_progress.h"
#include <SDL_main.h>
#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <UIKit/UIKit.h>
#include <mach/mach.h>
#include <sys/resource.h>

// Called at most once per five-second diagnostic window on the UIKit thread.
// No power estimate is derived from USB-charged battery readings.
typedef struct {
    double cpu_seconds;
    uint64_t resident_bytes;
    int32_t thermal, low_power, max_fps, battery_state, orientation;
    int32_t drawable_width, drawable_height;
} PeggleNativeDiagnostics;

void peggle_native_diagnostics(PeggleNativeDiagnostics *out) {
    struct rusage usage = {0};
    if (getrusage(RUSAGE_SELF, &usage) == 0)
        out->cpu_seconds = usage.ru_utime.tv_sec + usage.ru_utime.tv_usec / 1e6
                         + usage.ru_stime.tv_sec + usage.ru_stime.tv_usec / 1e6;
    mach_task_basic_info_data_t task = {0};
    mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
    if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&task, &count) == KERN_SUCCESS)
        out->resident_bytes = task.resident_size;
    NSProcessInfo *process = NSProcessInfo.processInfo;
    out->thermal = (int32_t)process.thermalState;
    out->low_power = process.lowPowerModeEnabled;
    UIWindowScene *scene = nil;
    for (UIScene *candidate in UIApplication.sharedApplication.connectedScenes)
        if ([candidate isKindOfClass:UIWindowScene.class] && candidate.activationState == UISceneActivationStateForegroundActive)
            scene = (UIWindowScene *)candidate;
    UIScreen *screen = scene.screen;
    out->max_fps = (int32_t)screen.maximumFramesPerSecond;
    out->battery_state = (int32_t)UIDevice.currentDevice.batteryState;
    out->orientation = (int32_t)scene.interfaceOrientation;
    SDL_Window *window = SDL_GetKeyboardFocus();
    if (window) SDL_GL_GetDrawableSize(window, &out->drawable_width, &out->drawable_height);
}

@interface PeggleSceneDelegate : NSObject <UIWindowSceneDelegate>
@property (nonatomic, strong) UIWindow *window;
@end

@implementation PeggleSceneDelegate
- (void)scene:(UIScene *)scene willConnectToSession:(UISceneSession *)session options:(UISceneConnectionOptions *)options {
    if (![scene isKindOfClass:UIWindowScene.class]) return;
    self.window = [[UIWindow alloc] initWithWindowScene:(UIWindowScene *)scene];
    self.window.rootViewController = [UIViewController new];
    self.window.rootViewController.view.backgroundColor = UIColor.blackColor;
    [self.window makeKeyAndVisible];
    NSLog(@"Peggle Compatibility scene connected");
}
- (void)sceneWillResignActive:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = UIApplication.sharedApplication.delegate;
    if ([delegate respondsToSelector:@selector(applicationWillResignActive:)])
        [delegate applicationWillResignActive:UIApplication.sharedApplication];
}
- (void)windowScene:(UIWindowScene *)windowScene didUpdateCoordinateSpace:(id<UICoordinateSpace>)previousCoordinateSpace interfaceOrientation:(UIInterfaceOrientation)previousInterfaceOrientation traitCollection:(UITraitCollection *)previousTraitCollection {
    NSLog(@"Peggle host orientation: %ld -> %ld; bounds %@", (long)previousInterfaceOrientation,
          (long)windowScene.interfaceOrientation, NSStringFromCGRect(windowScene.coordinateSpace.bounds));
}
- (void)sceneDidBecomeActive:(UIScene *)scene {
    PeggleAutoWake();
    id<UIApplicationDelegate> delegate = UIApplication.sharedApplication.delegate;
    if ([delegate respondsToSelector:@selector(applicationDidBecomeActive:)])
        [delegate applicationDidBecomeActive:UIApplication.sharedApplication];
}
- (void)sceneDidEnterBackground:(UIScene *)scene {
    PeggleAutoBackground();
    id<UIApplicationDelegate> delegate = UIApplication.sharedApplication.delegate;
    if ([delegate respondsToSelector:@selector(applicationDidEnterBackground:)])
        [delegate applicationDidEnterBackground:UIApplication.sharedApplication];
}
- (void)sceneWillEnterForeground:(UIScene *)scene {
    id<UIApplicationDelegate> delegate = UIApplication.sharedApplication.delegate;
    if ([delegate respondsToSelector:@selector(applicationWillEnterForeground:)])
        [delegate applicationWillEnterForeground:UIApplication.sharedApplication];
}
@end

extern int peggle_present_save_manager(NSString *documents);
extern int peggle_ios_run(const char *ipa, const char *resources, const char *documents);
extern void peggle_install_keyboard_bridge(void);
extern void peggle_install_fps_overlay(void);
extern void peggle_install_frame_pacing(void);

int SDL_main(int argc, char *argv[]) {
    @autoreleasepool {
        NSString *managerDocuments=NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,NSUserDomainMask,YES).firstObject;
        if(![[NSFileManager defaultManager]createDirectoryAtPath:managerDocuments withIntermediateDirectories:YES attributes:nil error:NULL])return 1;
        // Native save tools run before any guest thread/file handle exists.
        if(!peggle_present_save_manager(managerDocuments))return 1;
        peggle_install_keyboard_bridge();
        peggle_install_fps_overlay();
        peggle_install_frame_pacing();
        NSError *error = nil;
        AVAudioSession *audio = [AVAudioSession sharedInstance];
        [audio setCategory:AVAudioSessionCategoryAmbient error:&error];
        [audio setActive:YES error:&error];
        if (error) NSLog(@"Audio session: %@", error);
        NSString *resources = NSBundle.mainBundle.resourcePath;
        NSString *ipa = [resources stringByAppendingPathComponent:@"Peggle.ipa"];
        NSString *documents = NSSearchPathForDirectoriesInDomains(NSDocumentDirectory, NSUserDomainMask, YES).firstObject;
        int result;
        for(;;){
            PeggleAutoGameStarted();
            result=peggle_ios_run(ipa.fileSystemRepresentation, resources.fileSystemRepresentation, documents.fileSystemRepresentation);
            PeggleAutoGameStopped();
            if(result!=75)break;
            PeggleAutoPrepare(documents,YES);
            extern char *peggle_save_manager_action(int,const char *,int);extern void peggle_save_manager_free(char *);
            char *raw=peggle_save_manager_action(5,"",0);NSDictionary *started=raw?[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:raw]dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL]:nil;if(raw)peggle_save_manager_free(raw);
            if(![started[@"ok"]boolValue]){result=1;break;}
        }
        if(result!=0){
            __block BOOL dismissed=NO;
            UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Peggle could not continue" message:@"Close and reopen Peggle HD. Your previous builds remain available." preferredStyle:UIAlertControllerStyleAlert];
            [alert addAction:[UIAlertAction actionWithTitle:@"Close" style:UIAlertActionStyleDefault handler:^(UIAlertAction *action){dismissed=YES;}]];
            UIWindowScene *active=nil;for(UIScene *scene in UIApplication.sharedApplication.connectedScenes)if([scene isKindOfClass:UIWindowScene.class]&&scene.activationState==UISceneActivationStateForegroundActive){active=(UIWindowScene *)scene;break;}
            UIWindow *window=active.keyWindow;
            UIViewController *controller=window.rootViewController;while(controller.presentedViewController)controller=controller.presentedViewController;
            if(controller&&NSThread.isMainThread){[controller presentViewController:alert animated:YES completion:nil];while(!dismissed)[NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];}
        }
        return result;
    }
}

// Keep the host's UIKit entry point explicit instead of depending on an
// entry point pulled indirectly from the Rust static library's SDL objects.
#undef main
int main(int argc, char *argv[]) {
    return SDL_UIKitRunApp(argc, argv, SDL_main);
}
