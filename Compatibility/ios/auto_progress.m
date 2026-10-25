#import "auto_progress.h"
#import "cloud_progress.h"
#import <UIKit/UIKit.h>
#import <TargetConditionals.h>
#include <string.h>
#include <stdatomic.h>
#include <time.h>
extern char *peggle_save_manager_action(int,const char *,int);
extern void peggle_save_manager_free(char *);
static NSDictionary *Act(int code,NSString *value,BOOL yes){char *s=peggle_save_manager_action(code,value.UTF8String,yes);if(!s)return @{@"ok":@NO};NSData *d=[NSData dataWithBytes:s length:strlen(s)];peggle_save_manager_free(s);return [NSJSONSerialization JSONObjectWithData:d options:0 error:NULL]?:@{@"ok":@NO};}
static NSString *JSON(id v){return [[NSString alloc]initWithData:[NSJSONSerialization dataWithJSONObject:v options:NSJSONWritingSortedKeys error:NULL] encoding:NSUTF8StringEncoding];}
static NSDictionary *Merge(NSDictionary *v){return Act(20,JSON(v),NO);}
static NSString *Docs,*Status,*StartupIssue;
static NSDictionary *Local,*Pending;
static NSString *Deferred,*LastUploaded;
static BOOL Busy,Playing,Prompting,Reloading;
static NSDate *NextAttempt;
static NSUInteger Epoch;
static _Atomic unsigned LinkedID;
static _Atomic int PromptReady;
static NSTimeInterval LastCapture;
static NSString *StatePath(void){return [Docs stringByAppendingPathComponent:PeggleUsesFolderSync()?@"Folder-Sync-Auto-State.json":@"Progress-Auto-State.json"];}
static BOOL Enabled(void){return PeggleSyncReady()&&[NSUserDefaults.standardUserDefaults boolForKey:PeggleSyncPreferenceKey(@"PeggleAutomaticProgressSync")]&&[NSUserDefaults.standardUserDefaults integerForKey:PeggleSyncPreferenceKey(@"PeggleSyncProfileID")]>0;}
static void Persist(void){if(!Docs)return;NSMutableDictionary *v=[@{@"schema":@1,@"context":PeggleSyncContext(),@"profile_id":@(atomic_load(&LinkedID))}mutableCopy];if(Local)v[@"local"]=Local;if(Pending)v[@"pending"]=Pending;if(Deferred)v[@"deferred"]=Deferred;[JSON(v) writeToFile:StatePath() atomically:YES encoding:NSUTF8StringEncoding error:NULL];}
static BOOL Receive(NSDictionary *remote){
 NSString *before=Pending?JSON(Pending):nil;
 if(!Local||!remote)return NO;NSDictionary *m=Merge(@{@"local":Local[@"document"],@"remote":remote});if(![m[@"ok"]boolValue])return NO;
 if([m[@"remote_adds"]boolValue]){Pending=m[@"document"];BOOL novel=YES;if(Deferred){NSDictionary *old=[NSJSONSerialization JSONObjectWithData:[Deferred dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];if(old){NSDictionary *covered=Merge(@{@"local":Local[@"document"],@"remote":old});if([covered[@"ok"]boolValue]){NSDictionary *delta=Merge(@{@"local":covered[@"document"],@"remote":Pending});if([delta[@"ok"]boolValue])novel=[delta[@"remote_adds"]boolValue];}}}atomic_store(&PromptReady,novel?1:0);}else{Pending=nil;Deferred=nil;atomic_store(&PromptReady,0);}NSString *after=Pending?JSON(Pending):nil;if(![before isEqual:after]&&(before||after))Persist();return YES;
}
static void Exchange(BOOL force){
 if(!Enabled()||Busy||!Local||Reloading)return;
 if(!force&&NextAttempt&&[NextAttempt timeIntervalSinceNow]>0)return;
 Busy=YES;NSUInteger epoch=Epoch;NSDictionary *source=Local;Status=PeggleUsesFolderSync()?@"Checking save files in the selected folder…":@"Syncing save files with iCloud…";
#if TARGET_OS_SIMULATOR
 // Explicit simulator-only fixture transport. Device builds contain no path
 // for injecting cloud documents or synthetic progress.
 NSString *testPath=NSProcessInfo.processInfo.environment[@"PEGGLE_AUTO_TEST_REMOTE"];
 if(testPath){NSDictionary *remote=[NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:testPath]?:[NSData data] options:0 error:NULL];if(remote)Receive(remote);Busy=NO;NextAttempt=[NSDate dateWithTimeIntervalSinceNow:2];LastUploaded=JSON(source[@"document"]);Status=Pending?@"An updated iCloud save file is ready to apply.":(PeggleUsesFolderSync()?@"Save files were checked in the selected folder. iCloud Drive controls when changes reach your other devices.":@"Adventure checkpoints, completed levels, and trophies synced with iCloud.");return;}
#endif
 PeggleCloudExchange(source,^NSDictionary *(NSDictionary *v){return Merge(v);},^(NSDictionary *result){dispatch_async(dispatch_get_main_queue(),^{
  if(epoch!=Epoch)return;Busy=NO;if(!Enabled())return;
  if([result[@"ok"]boolValue]){LastUploaded=JSON(source[@"document"]);NextAttempt=[NSDate dateWithTimeIntervalSinceNow:30];Receive(result[@"document"]);if(![JSON(Local[@"document"])isEqual:LastUploaded])NextAttempt=nil;Status=Pending?@"An updated iCloud save file is ready to apply.":(PeggleUsesFolderSync()?@"Save files were checked in the selected folder. iCloud Drive controls when changes reach your other devices.":@"Adventure checkpoints, completed levels, and trophies synced with iCloud.");}
  else{NextAttempt=[NSDate dateWithTimeIntervalSinceNow:15];Status=@"Your save files are stored on this device. iCloud sync will retry automatically.";StartupIssue=result[@"error"]?:Status;Persist();}
 });});
}
static void AcceptSnapshot(NSDictionary *snapshot){
 if(!Enabled()||[snapshot[@"profile_id"]unsignedIntValue]!=atomic_load(&LinkedID))return;
 BOOL changed=!Local||![JSON(Local[@"document"])isEqual:JSON(snapshot[@"document"])];Local=snapshot;
 if(Pending)Receive(Pending);if(changed)Persist();
 // Changed completed saves upload promptly; an unchanged snapshot only polls
 // once per 30 seconds while foreground. Failures retain the retry deadline.
 if(changed&&LastUploaded&&![JSON(snapshot[@"document"])isEqual:LastUploaded])NextAttempt=nil;
 Exchange(NO);
}
// Interpreter calls this on its native parent stack while guest execution is
// stopped and no userdata writer is open. Copy JSON before returning.
void peggle_auto_snapshot(const char *json){if(!json)return;NSDictionary *v=[NSJSONSerialization JSONObjectWithData:[[NSString stringWithUTF8String:json]dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];if(v)AcceptSnapshot(v);}
unsigned peggle_auto_capture_due(void){if(!Playing||!Enabled())return 0;NSTimeInterval now=NSProcessInfo.processInfo.systemUptime;if(now-LastCapture<2)return 0;LastCapture=now;return atomic_load(&LinkedID);}
unsigned peggle_auto_profile(void){return Enabled()?atomic_load(&LinkedID):0;}
int peggle_auto_prompt_due(void){return Playing&&Enabled()&&!Prompting&&UIApplication.sharedApplication.applicationState==UIApplicationStateActive&&atomic_load(&PromptReady);}
// Invoked with the guest paused, audio suspended, and its save callbacks done.
int peggle_auto_prompt(void){
 if(!Pending||!Local||!Enabled())return 0;Prompting=YES;__block int choice=-1;
 UIWindowScene *scene=nil;for(UIScene *s in UIApplication.sharedApplication.connectedScenes)if([s isKindOfClass:UIWindowScene.class]&&s.activationState==UISceneActivationStateForegroundActive){scene=(UIWindowScene *)s;break;}
 if(!scene){Prompting=NO;return 0;}
 UIWindow *window=[[UIWindow alloc]initWithWindowScene:scene];window.tintColor=[UIColor colorWithRed:246.0/255 green:117.0/255 blue:39.0/255 alpha:1];window.windowLevel=UIWindowLevelAlert+1;window.rootViewController=[UIViewController new];[window makeKeyAndVisible];
 UIAlertController *a=[UIAlertController alertControllerWithTitle:@"Updated iCloud Save File" message:@"Another device has saved a later Adventure checkpoint, more cleared or aced levels, or additional challenge trophies. Apply these changes and restart Peggle, or keep playing and apply them next time you open the game? A later Adventure checkpoint replaces an older unfinished board and starts that level from the beginning." preferredStyle:UIAlertControllerStyleAlert];
 [a addAction:[UIAlertAction actionWithTitle:@"Later" style:UIAlertActionStyleCancel handler:^(UIAlertAction *x){Deferred=JSON(Pending);atomic_store(&PromptReady,0);Persist();NSLog(@"PEGGLE_AUTO chose_later");choice=0;}]];
 [a addAction:[UIAlertAction actionWithTitle:@"Apply and Restart" style:UIAlertActionStyleDefault handler:^(UIAlertAction *x){Persist();Reloading=YES;atomic_store(&PromptReady,0);NSLog(@"PEGGLE_AUTO chose_apply_now");choice=1;}]];
 [window.rootViewController presentViewController:a animated:YES completion:nil];
 while(choice<0){@autoreleasepool{[NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:.02]];}}
 window.hidden=YES;window.rootViewController=nil;Prompting=NO;return choice;
}
NSString *PeggleAutoStatus(void){return Status;}
NSString *PeggleAutoStartupIssue(void){return StartupIssue;}
void PeggleAutoGameStarted(void){Playing=YES;LastCapture=0;NSLog(@"PEGGLE_AUTO guest_started");}
void PeggleAutoGameStopped(void){Playing=NO;Reloading=NO;NSLog(@"PEGGLE_AUTO guest_stopped");}
void PeggleAutoPrepare(NSString *documents,BOOL reload){
 StartupIssue=nil;Docs=documents;Playing=NO;Reloading=NO;atomic_store(&LinkedID,(unsigned)[NSUserDefaults.standardUserDefaults integerForKey:PeggleSyncPreferenceKey(@"PeggleSyncProfileID")]);if(!Enabled()){Status=nil;return;}
 NSDictionary *source=Act(10,@"",NO);if(![source[@"ok"]boolValue]||[source[@"profile_id"]unsignedIntValue]!=atomic_load(&LinkedID)){Status=PeggleUsesFolderSync()?@"Automatic sync is waiting for its linked in-game user. Select that user inside Peggle, then close and reopen the app. To link the selected user instead, turn iCloud sync off and on in Manage Save Files. This combines that user’s completed levels and trophies with the selected folder.":@"Automatic sync is waiting for its linked in-game user. Select that user inside Peggle, then close and reopen the app. To link the selected user instead, turn iCloud sync off and on in Manage Save Files; this combines that user’s completed levels and trophies with the shared iCloud save file.";StartupIssue=Status;return;}
 if(!Local){NSDictionary *saved=[NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:StatePath()]?:[NSData data] options:0 error:NULL];if((!PeggleUsesFolderSync()||[saved[@"context"]isEqual:PeggleSyncContext()])&&[saved[@"profile_id"]unsignedIntValue]==atomic_load(&LinkedID)){Pending=saved[@"pending"];Deferred=saved[@"deferred"];}}
 Local=source;
 // Revalidate staged cloud data against freshly saved local progress. Apply
 // only here, after the previous Environment and its file handles are gone.
 if(Pending){NSDictionary *preview=Act(13,JSON(Pending),NO);if([preview[@"ok"]boolValue]){NSDictionary *result=[preview[@"changed"]boolValue]?Act(12,preview[@"token"],YES):Act(7,@"",NO);if([result[@"ok"]boolValue]){Local=Act(10,@"",NO);Pending=nil;Deferred=nil;atomic_store(&PromptReady,0);Persist();}else{StartupIssue=result[@"error"];}}else{StartupIssue=preview[@"error"];}}
 if(!reload){
  Exchange(YES);NSTimeInterval end=NSProcessInfo.processInfo.systemUptime+3;
  while(Busy&&NSProcessInfo.processInfo.systemUptime<end){@autoreleasepool{[NSRunLoop.mainRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:.025]];}}
  if(!Busy&&Pending){NSDictionary *preview=Act(13,JSON(Pending),NO);if([preview[@"ok"]boolValue]){NSDictionary *result=[preview[@"changed"]boolValue]?Act(12,preview[@"token"],YES):Act(7,@"",NO);if([result[@"ok"]boolValue]){Local=Act(10,@"",NO);Pending=nil;Deferred=nil;atomic_store(&PromptReady,0);Persist();}else{StartupIssue=result[@"error"];}}else{StartupIssue=preview[@"error"];}}
 }
}
void PeggleAutoDisconnect(void){
 Epoch++;Busy=NO;Local=nil;Pending=nil;Deferred=nil;LastUploaded=nil;StartupIssue=nil;NextAttempt=nil;Status=nil;
 atomic_store(&LinkedID,0);atomic_store(&PromptReady,0);
 NSUserDefaults *d=NSUserDefaults.standardUserDefaults;[d setBool:NO forKey:PeggleSyncPreferenceKey(@"PeggleAutomaticProgressSync")];[d removeObjectForKey:PeggleSyncPreferenceKey(@"PeggleSyncProfileID")];Persist();
}
void PeggleAutoEnable(void(^completion)(NSString *)){
 if(!PeggleSyncReady()){completion(PeggleUsesFolderSync()?@"Choose an iCloud Drive folder in Manage Save Files before enabling sync.":@"CloudKit is unavailable in this build. Export and import a save file to test transfers.");return;}
 NSDictionary *source=Act(10,@"",NO);if(![source[@"ok"]boolValue]){completion(@"Start the game and create an in-game user before syncing save files.");return;}
 NSUserDefaults *d=NSUserDefaults.standardUserDefaults;[d setBool:YES forKey:PeggleSyncPreferenceKey(@"PeggleAutomaticProgressSync")];[d setBool:NO forKey:@"PeggleShowSaveManagementOnLaunch"];[d setInteger:[source[@"profile_id"]integerValue]forKey:PeggleSyncPreferenceKey(@"PeggleSyncProfileID")];Epoch++;Local=source;Pending=nil;Deferred=nil;LastUploaded=nil;atomic_store(&LinkedID,[source[@"profile_id"]unsignedIntValue]);atomic_store(&PromptReady,0);Persist();PeggleAutoPrepare(Docs,NO);if(PeggleUsesFolderSync()&&StartupIssue){completion(StartupIssue);return;}completion([NSString stringWithFormat:@"Automatic sync enabled for in-game user %@. %@",source[@"profile"],Status]);
}

void PeggleAutoWake(void){NextAttempt=nil;LastCapture=0;}
void PeggleAutoBackground(void){
 if(!Enabled())return;
 // A bounded courtesy window for the original game's imminent save callback.
 // The durable local document still supports retry if iOS suspends us sooner.
 __block UIBackgroundTaskIdentifier task=UIBackgroundTaskInvalid;
 void(^finish)(void)=^{if(task!=UIBackgroundTaskInvalid){[UIApplication.sharedApplication endBackgroundTask:task];task=UIBackgroundTaskInvalid;}};
 task=[UIApplication.sharedApplication beginBackgroundTaskWithName:@"Sync Peggle progress" expirationHandler:finish];
 dispatch_after(dispatch_time(DISPATCH_TIME_NOW,25*NSEC_PER_SEC),dispatch_get_main_queue(),finish);
 NextAttempt=nil;LastCapture=0;
}
