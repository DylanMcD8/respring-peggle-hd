#import "cloud_progress.h"
#import <CloudKit/CloudKit.h>
#include <math.h>
NSString * const PeggleCloudSyncDidCompleteNotification=@"PeggleCloudSyncDidComplete";
static NSString * const LastSyncKey=@"PeggleLastSuccessfulCloudSync";
NSDate *PeggleCloudLastSyncDate(void) {
 NSUserDefaults *defaults=NSUserDefaults.standardUserDefaults;
 id saved=[defaults objectForKey:PeggleUsesFolderSync()?@"PeggleLastFolderExchange":LastSyncKey];
 // Seed older installations only from a receipt that confirms a successful
 // cloud exchange. Failed attempts must never become the last-sync date.
 if(!saved&&!PeggleUsesFolderSync()){
  NSString *documents=NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,NSUserDomainMask,YES).firstObject;
  NSDictionary *receipt=[NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:[documents stringByAppendingPathComponent:@"Progress-sync-status.json"]]?:[NSData data] options:0 error:NULL];
  if([receipt isKindOfClass:NSDictionary.class]&&[receipt[@"ok"]isKindOfClass:NSNumber.class]&&[receipt[@"cloud_synced"]isKindOfClass:NSNumber.class]&&[receipt[@"ok"]boolValue]&&[receipt[@"cloud_synced"]boolValue])saved=receipt[@"timestamp"];
  if([saved isKindOfClass:NSNumber.class]&&isfinite([saved doubleValue])&&[saved doubleValue]>0)[defaults setObject:saved forKey:LastSyncKey];
 }
 if(![saved isKindOfClass:NSNumber.class]||!isfinite([saved doubleValue])||[saved doubleValue]<=0)return nil;
 return [NSDate dateWithTimeIntervalSince1970:[saved doubleValue]];
}
static void RecordSuccessfulSync(void) {
 [NSUserDefaults.standardUserDefaults setDouble:NSDate.date.timeIntervalSince1970 forKey:PeggleUsesFolderSync()?@"PeggleLastFolderExchange":LastSyncKey];
 dispatch_async(dispatch_get_main_queue(),^{[NSNotificationCenter.defaultCenter postNotificationName:PeggleCloudSyncDidCompleteNotification object:nil];});
}
// One explicitly selected shared progress slot for the prototype. Only our JSON
// document enters CloudKit; no guest paths, preferences or original saves do.
@interface PeggleProgressCloudSession:NSObject
@property(copy) NSDictionary *(^action)(int,NSString *,BOOL);
@property(copy) void(^completion)(NSDictionary *);
@property(strong) CKDatabase *database;
@property(strong) CKRecordID *recordID;
@property NSInteger attempts;
@end
@implementation PeggleProgressCloudSession
- (void)finish:(NSDictionary *)result {
 (void)PeggleCloudLastSyncDate(); // Preserve an older successful receipt before replacing it.
 if([result[@"ok"]boolValue]&&[result[@"cloud_synced"]boolValue])RecordSuccessfulSync();
 NSString *documents=NSSearchPathForDirectoriesInDomains(NSDocumentDirectory,NSUserDomainMask,YES).firstObject;
 NSMutableDictionary *receipt=[result mutableCopy];receipt[@"timestamp"]=@([NSDate.date timeIntervalSince1970]);receipt[@"app"]=NSBundle.mainBundle.bundleIdentifier;
 [[NSJSONSerialization dataWithJSONObject:receipt options:NSJSONWritingSortedKeys error:NULL] writeToFile:[documents stringByAppendingPathComponent:@"Progress-sync-status.json"] atomically:YES];
 void(^done)(NSDictionary *)=self.completion;self.completion=nil;if(done)done(result);}
- (void)fail:(NSError *)error {NSString *message=error.localizedDescription?:@"The shared iCloud save file could not be synced.";[self finish:@{@"ok":@NO,@"error":[message stringByAppendingString:@" Your local save files are retained. You can continue playing offline and retry later."]}];}
- (NSError *)specificError:(NSError *)error {if(error.code==CKErrorPartialFailure){NSError *one=[error.userInfo[CKPartialErrorsByItemIDKey] objectForKey:self.recordID];if(one)return one;}return error;}
- (void)configure:(CKOperation *)operation {CKOperationConfiguration *configuration=[CKOperationConfiguration new];configuration.timeoutIntervalForRequest=20;configuration.timeoutIntervalForResource=45;configuration.qualityOfService=NSQualityOfServiceUserInitiated;operation.configuration=configuration;}
- (void)fetch {
 self.attempts++;
 CKFetchRecordsOperation *operation=[[CKFetchRecordsOperation alloc]initWithRecordIDs:@[self.recordID]];[self configure:operation];
 operation.fetchRecordsCompletionBlock=^(NSDictionary<CKRecordID *,CKRecord *> *records,NSError *error){
  error=[self specificError:error];if(error&&error.code!=CKErrorUnknownItem){[self fail:error];return;}
  CKRecord *record=records[self.recordID];NSDictionary *source=self.action(10,@"",NO);if(![source[@"ok"]boolValue]){[self finish:source];return;}
  NSDictionary *incoming=source[@"document"];
  if(record){id payload=record[@"progressJSON"];if(![payload isKindOfClass:NSData.class]||[payload length]>512*1024){[self finish:@{@"ok":@NO,@"error":@"This version of the iCloud save file is not supported. Nothing was changed."}];return;}incoming=[NSJSONSerialization JSONObjectWithData:payload options:0 error:NULL];if(![incoming isKindOfClass:NSDictionary.class]){[self finish:@{@"ok":@NO,@"error":@"The iCloud save file is damaged. Nothing was changed."}];return;}}
  NSData *data=[NSJSONSerialization dataWithJSONObject:incoming options:0 error:NULL];NSDictionary *preview=self.action(13,[[NSString alloc]initWithData:data encoding:NSUTF8StringEncoding],NO);if(![preview[@"ok"]boolValue]){[self finish:preview];return;}
  NSData *merged=[NSJSONSerialization dataWithJSONObject:preview[@"document"] options:NSJSONWritingSortedKeys error:NULL];
  if(!record)record=[[CKRecord alloc]initWithRecordType:@"PeggleProgress" recordID:self.recordID];record[@"progressJSON"]=merged;record[@"schemaVersion"]=preview[@"document"][@"schema"];
  CKModifyRecordsOperation *save=[[CKModifyRecordsOperation alloc]initWithRecordsToSave:@[record] recordIDsToDelete:nil];save.savePolicy=CKRecordSaveIfServerRecordUnchanged;save.atomic=YES;[self configure:save];
  save.modifyRecordsCompletionBlock=^(NSArray *saved,NSArray *deleted,NSError *saveError){
   saveError=[self specificError:saveError];if(saveError){if(saveError.code==CKErrorServerRecordChanged&&self.attempts<3){[self fetch];return;}[self fail:saveError];return;}
   // Upload completes before local publication. A failed local commit can be
   // retried without losing the previous selection or duplicating counters.
   NSDictionary *applied=[preview[@"changed"]boolValue]?self.action(12,preview[@"token"],YES):self.action(7,@"",NO);
   if(![applied[@"ok"]boolValue]){[self finish:applied];return;}
   NSMutableDictionary *result=[applied mutableCopy];result[@"profile"]=preview[@"profile"];result[@"summary"]=preview[@"after"];result[@"cloud_synced"]=@YES;result[@"changed"]=preview[@"changed"];[self finish:result];
  };if(preview[@"upload_needed"]&&![preview[@"upload_needed"]boolValue]){save.modifyRecordsCompletionBlock(@[],@[],nil);}else{[self.database addOperation:save];}
 };[self.database addOperation:operation];
}
@end
void PeggleCloudProgress(NSDictionary *(^action)(int,NSString *,BOOL),void(^completion)(NSDictionary *)) {
 if(PeggleUsesFolderSync()){
  NSDictionary *source=action(10,@"",NO);if(![source[@"ok"]boolValue]){completion(source);return;}
  PeggleFolderExchange(source,^NSDictionary *(NSDictionary *value){NSData *data=[NSJSONSerialization dataWithJSONObject:value options:0 error:NULL];return action(20,[[NSString alloc]initWithData:data encoding:NSUTF8StringEncoding],NO);},^(NSDictionary *result){
   if(![result[@"ok"]boolValue]){completion(result);return;}
   NSData *data=[NSJSONSerialization dataWithJSONObject:result[@"document"] options:0 error:NULL];NSDictionary *preview=action(13,[[NSString alloc]initWithData:data encoding:NSUTF8StringEncoding],NO);
   if(![preview[@"ok"]boolValue]){completion(preview);return;}
   NSDictionary *applied=[preview[@"changed"]boolValue]?action(12,preview[@"token"],YES):action(7,@"",NO);
   if(![applied[@"ok"]boolValue]){completion(applied);return;}
   NSMutableDictionary *done=[applied mutableCopy];done[@"profile"]=source[@"profile"]?:@"";done[@"folder_checked"]=@YES;RecordSuccessfulSync();completion(done);
  });return;
 }
 if(!PeggleCloudSupported()){completion(@{@"ok":@NO,@"error":@"CloudKit is unavailable in this build. Export and import a save file to test transfers."});return;}
 CKContainer *container=[CKContainer containerWithIdentifier:[NSBundle.mainBundle objectForInfoDictionaryKey:@"RespringCloudKitContainer"]?:@"iCloud.com.respringapps.peggle"];
 PeggleProgressCloudSession *session=[PeggleProgressCloudSession new];session.action=action;session.completion=completion;session.database=container.privateCloudDatabase;session.recordID=[[CKRecordID alloc]initWithRecordName:@"prototype-unlocks-v1"];
 [container accountStatusWithCompletionHandler:^(CKAccountStatus status,NSError *error){if(error){[session fail:error];return;}if(status!=CKAccountStatusAvailable){[session finish:@{@"ok":@NO,@"error":@"Sign in to iCloud in Settings to sync save files. You can continue playing offline."}];return;}[session fetch];}];
}

// Runtime transport uses immutable canonical documents only. It cannot invoke
// Save Manager's live-file operations or change the selected save slot.
void PeggleCloudExchange(NSDictionary *source,NSDictionary *(^pureMerge)(NSDictionary *),void(^completion)(NSDictionary *)) {
 if(PeggleUsesFolderSync()){PeggleFolderExchange(source,pureMerge,^(NSDictionary *result){if([result[@"ok"]boolValue])RecordSuccessfulSync();completion(result);});return;}
 __block NSDictionary *merged=nil;
 PeggleCloudProgress(^NSDictionary *(int code,NSString *value,BOOL confirmed){
  if(code==10)return source;
  if(code==13){NSDictionary *remote=[NSJSONSerialization JSONObjectWithData:[value dataUsingEncoding:NSUTF8StringEncoding] options:0 error:NULL];NSDictionary *r=pureMerge(@{@"local":source[@"document"],@"remote":remote?:@{}});if(![r[@"ok"]boolValue])return r;merged=r[@"document"];return @{@"ok":@YES,@"changed":r[@"remote_adds"],@"upload_needed":r[@"upload_needed"],@"document":merged,@"after":r[@"summary"],@"profile":source[@"profile"],@"token":@"canonical-only"};}
  if(code==7||code==12)return @{@"ok":@YES,@"document":merged?:@{}};
  return @{@"ok":@NO,@"error":@"Unsupported automatic sync action"};
 },completion);
}
