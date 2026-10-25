#import "folder_sync.h"
#import "cloud_capability.h"
#import <TargetConditionals.h>
#include <stdatomic.h>

static NSString * const BookmarkKey=@"PeggleSyncFolderBookmark";
static NSString * const ContextKey=@"PeggleSyncFolderSelection";
static NSString * const NameKey=@"PeggleSyncFolderName";
static NSString * const Prefix=@"Peggle-Save-";
static const NSUInteger FileLimit=512*1024, CountLimit=128, TotalLimit=8*1024*1024;
BOOL PeggleUsesFolderSync(void) {return ![[NSBundle.mainBundle objectForInfoDictionaryKey:@"RespringProgressCloudKit"]boolValue];}
BOOL PeggleSyncSupported(void) {return PeggleUsesFolderSync()||PeggleCloudSupported();}
BOOL PeggleSyncReady(void) {return PeggleUsesFolderSync()?[NSUserDefaults.standardUserDefaults dataForKey:BookmarkKey].length>0:PeggleCloudSupported();}
NSString *PeggleSyncPreferenceKey(NSString *key) {return PeggleUsesFolderSync()?[key stringByAppendingString:@"Folder"]:key;}
NSString *PeggleSyncContext(void) {return PeggleUsesFolderSync()?([NSUserDefaults.standardUserDefaults stringForKey:ContextKey]?:@"folder-unselected"):@"cloudkit";}
NSString *PeggleFolderName(void) {return [NSUserDefaults.standardUserDefaults stringForKey:NameKey];}
static dispatch_queue_t FolderQueue(void) {static dispatch_queue_t queue;static dispatch_once_t once;dispatch_once(&once,^{queue=dispatch_queue_create("com.respringapps.peggle.folder-sync",DISPATCH_QUEUE_SERIAL);});return queue;}
static NSError *Problem(NSString *message) {return [NSError errorWithDomain:@"PeggleFolderSync" code:1 userInfo:@{NSLocalizedDescriptionKey:message}];}
static NSDictionary *Failure(NSError *error) {return @{@"ok":@NO,@"error":error.localizedDescription?:@"The selected folder could not be accessed. Your local save files are unchanged."};}
static BOOL IsDirectory(NSURL *url,NSError **error) {
 NSDictionary *values=[url resourceValuesForKeys:@[NSURLIsDirectoryKey,NSURLIsSymbolicLinkKey] error:error];
 if(!values)return NO;
 if(![values[NSURLIsDirectoryKey]boolValue]||[values[NSURLIsSymbolicLinkKey]boolValue]){if(error)*error=Problem(@"Choose a folder in iCloud Drive.");return NO;}return YES;
}
void PeggleSelectSyncFolder(NSURL *url,void(^completion)(NSError *)) {
 dispatch_async(FolderQueue(),^{@autoreleasepool {
  BOOL scoped=[url startAccessingSecurityScopedResource];NSError *error=nil;NSData *bookmark=nil;
  if(!scoped)error=Problem(@"Access to this folder was not granted. Choose the folder again in Files.");
  else if(IsDirectory(url,&error)) {
   NSNumber *ubiquitous=nil;[url getResourceValue:&ubiquitous forKey:NSURLIsUbiquitousItemKey error:&error];
   if(!error&&![ubiquitous boolValue])error=Problem(@"Choose a folder in iCloud Drive so the save files can reach your other devices.");
   if(!error)bookmark=[url bookmarkDataWithOptions:NSURLBookmarkCreationMinimalBookmark includingResourceValuesForKeys:nil relativeToURL:nil error:&error];
  }
  if(scoped)[url stopAccessingSecurityScopedResource];
  dispatch_async(dispatch_get_main_queue(),^{
   if(bookmark){NSUserDefaults *d=NSUserDefaults.standardUserDefaults;[d setObject:bookmark forKey:BookmarkKey];[d setObject:NSUUID.UUID.UUIDString forKey:ContextKey];[d setObject:url.lastPathComponent forKey:NameKey];[d removeObjectForKey:@"PeggleLastFolderExchange"];}
   completion(error?:(!bookmark?Problem(@"The folder could not be remembered. Choose it again in Files."):nil));
  });
 }});
}
static NSString *WriterID(NSError **error) {
 NSURL *library=[NSFileManager.defaultManager URLsForDirectory:NSApplicationSupportDirectory inDomains:NSUserDomainMask].firstObject;
 NSURL *directory=[library URLByAppendingPathComponent:@"Peggle Folder Sync" isDirectory:YES];
 if(![NSFileManager.defaultManager createDirectoryAtURL:directory withIntermediateDirectories:YES attributes:nil error:error])return nil;
 // A restored backup must get a new writer, not compete with the old device.
 if(![directory setResourceValue:@YES forKey:NSURLIsExcludedFromBackupKey error:error])return nil;
 NSURL *path=[directory URLByAppendingPathComponent:@"Writer-ID.txt"];
 NSString *identifier=[NSString stringWithContentsOfURL:path encoding:NSUTF8StringEncoding error:NULL];
 if(![[NSUUID alloc]initWithUUIDString:identifier]){identifier=NSUUID.UUID.UUIDString;if(![identifier writeToURL:path atomically:YES encoding:NSUTF8StringEncoding error:error])return nil;}
 return identifier;
}
static NSDictionary *ReadDocument(NSURL *url,NSUInteger *total,NSError **error) {
 NSDictionary *v=[url resourceValuesForKeys:@[NSURLIsRegularFileKey,NSURLIsSymbolicLinkKey,NSURLFileSizeKey,NSURLIsUbiquitousItemKey,NSURLUbiquitousItemDownloadingStatusKey] error:error];
 if(!v)return nil;
 if(![v[NSURLIsRegularFileKey]boolValue]||[v[NSURLIsSymbolicLinkKey]boolValue]){*error=Problem(@"A sync save file is not a regular file. Nothing was changed.");return nil;}
 if([v[NSURLIsUbiquitousItemKey]boolValue]&&[v[NSURLUbiquitousItemDownloadingStatusKey]isEqual:NSURLUbiquitousItemDownloadingStatusNotDownloaded]){
  [NSFileManager.defaultManager startDownloadingUbiquitousItemAtURL:url error:NULL];*error=Problem(@"Save files are downloading from iCloud Drive. Keep playing and syncing will retry automatically, or try Sync Immediately again later.");return nil;
 }
 NSUInteger size=[v[NSURLFileSizeKey]unsignedIntegerValue];
 if(size>FileLimit||size>TotalLimit-*total){*error=Problem(@"The selected folder contains too much sync save file data. Nothing was changed.");return nil;}
 // Bounded read even if the file changes between resource lookup and opening.
 NSFileHandle *handle=[NSFileHandle fileHandleForReadingFromURL:url error:error];if(!handle)return nil;
 NSData *data=[handle readDataUpToLength:FileLimit+1 error:error];[handle closeAndReturnError:NULL];if(!data)return nil;
 if(data.length>FileLimit||data.length>TotalLimit-*total){*error=Problem(@"A sync save file exceeds the supported size. Nothing was changed.");return nil;}*total+=data.length;
 id document=[NSJSONSerialization JSONObjectWithData:data options:0 error:error];
 if(![document isKindOfClass:NSDictionary.class]){*error=Problem(@"A sync save file could not be read. Nothing was changed.");return nil;}return document;
}
static NSDictionary *Merge(NSDictionary *local,NSDictionary *remote,NSDictionary *(^merge)(NSDictionary *),NSError **error) {
 NSDictionary *result=merge(@{@"local":local?:@{},@"remote":remote?:@{}});
 if(![result[@"ok"]boolValue]||![result[@"document"]isKindOfClass:NSDictionary.class]){*error=Problem(result[@"error"]?:@"A sync save file uses an unsupported format. Nothing was changed.");return nil;}return result[@"document"];
}
NSDictionary *PeggleFolderExchangeAtURL(NSURL *folder,NSString *writer,NSDictionary *source,NSDictionary *(^merge)(NSDictionary *)) {
 if(![[NSUUID alloc]initWithUUIDString:writer])return Failure(Problem(@"The local sync save file could not be identified. Restart the app and try again."));
 __block NSError *error=nil;__block NSDictionary *merged=Merge(source[@"document"],source[@"document"],merge,&error);if(!merged)return Failure(error);
 if(!IsDirectory(folder,&error))return Failure(error);
 NSFileCoordinator *coordinator=[[NSFileCoordinator alloc]initWithFilePresenter:nil];
 __block atomic_bool finished=false;
 dispatch_after(dispatch_time(DISPATCH_TIME_NOW,25*NSEC_PER_SEC),dispatch_get_global_queue(QOS_CLASS_UTILITY,0),^{if(!atomic_load(&finished))[coordinator cancel];});
 @try {
  __block NSArray<NSURL *> *files=nil;NSError *coordinationError=nil;
  [coordinator coordinateReadingItemAtURL:folder options:0 error:&coordinationError byAccessor:^(NSURL *url){files=[NSFileManager.defaultManager contentsOfDirectoryAtURL:url includingPropertiesForKeys:nil options:0 error:&error];}];
  if(!files||error||coordinationError)return Failure(error?:coordinationError);
  NSPredicate *managed=[NSPredicate predicateWithBlock:^BOOL(NSURL *url,NSDictionary *bindings){return [url.lastPathComponent hasPrefix:Prefix]&&[url.pathExtension.lowercaseString isEqual:@"json"];}];
  files=[[files filteredArrayUsingPredicate:managed]sortedArrayUsingComparator:^NSComparisonResult(NSURL *a,NSURL *b){return [a.lastPathComponent compare:b.lastPathComponent];}];
  if(files.count>CountLimit)return Failure(Problem(@"The selected folder contains too many sync save files. Nothing was changed."));
  __block NSUInteger total=0;NSTimeInterval deadline=NSProcessInfo.processInfo.systemUptime+25;
  for(NSURL *file in files) {
   if(NSProcessInfo.processInfo.systemUptime>deadline)return Failure(Problem(@"iCloud Drive is taking longer than expected. Your local save files are retained; syncing will retry."));
   NSNumber *link=nil;if(![file getResourceValue:&link forKey:NSURLIsSymbolicLinkKey error:&error]||link.boolValue)return Failure(error?:Problem(@"A sync save file is not a regular file. Nothing was changed."));
   coordinationError=nil;
   [coordinator coordinateReadingItemAtURL:file options:0 error:&coordinationError byAccessor:^(NSURL *url){
    NSDictionary *document=ReadDocument(url,&total,&error);if(!document)return;merged=Merge(merged,document,merge,&error);if(!merged)return;
    // Usually each installation owns a different file. Preserve and merge any
    // provider conflict versions too; never discard a version we haven't read.
    NSArray<NSFileVersion *> *versions=[NSFileVersion unresolvedConflictVersionsOfItemAtURL:url];
    if(versions.count>16){error=Problem(@"A sync save file has too many conflicting versions. Nothing was changed.");return;}
    for(NSFileVersion *version in versions){NSDictionary *conflict=ReadDocument(version.URL,&total,&error);if(!conflict)return;merged=Merge(merged,conflict,merge,&error);if(!merged)return;}
   }];
   if(error||coordinationError||!merged)return Failure(error?:coordinationError);
  }
  NSURL *target=[folder URLByAppendingPathComponent:[NSString stringWithFormat:@"%@%@.json",Prefix,writer]];
  if(files.count>=CountLimit&&![[files valueForKey:@"lastPathComponent"]containsObject:target.lastPathComponent])return Failure(Problem(@"The selected folder contains too many sync save files. Nothing was changed."));
  if([NSFileManager.defaultManager fileExistsAtPath:target.path]){NSNumber *link=nil;if(![target getResourceValue:&link forKey:NSURLIsSymbolicLinkKey error:&error]||link.boolValue)return Failure(error?:Problem(@"A sync save file is not a regular file. Nothing was changed."));}
  coordinationError=nil;__block BOOL published=NO;
  [coordinator coordinateWritingItemAtURL:target options:0 error:&coordinationError byAccessor:^(NSURL *url){
   // Re-read our own file under the write claim; never overwrite a newer copy
   // between directory enumeration and publication.
   if([NSFileManager.defaultManager fileExistsAtPath:url.path]){NSDictionary *current=ReadDocument(url,&total,&error);if(!current)return;merged=Merge(merged,current,merge,&error);if(!merged)return;}
   NSMutableDictionary *outgoing=[merged mutableCopy];outgoing[@"profile_label"]=@"Peggle";
   NSData *data=[NSJSONSerialization dataWithJSONObject:outgoing options:NSJSONWritingSortedKeys error:&error];if(!data)return;
   if(data.length>FileLimit){error=Problem(@"The merged sync save file exceeds the supported size. Nothing was changed.");return;}
   NSData *existing=[NSData dataWithContentsOfURL:url options:NSDataReadingMappedIfSafe error:NULL];
   if(![existing isEqual:data]&&![data writeToURL:url options:NSDataWritingAtomic error:&error])return;
   published=YES;
  }];
  if(error||coordinationError||!published)return Failure(error?:coordinationError);
  return @{@"ok":@YES,@"document":merged,@"profile":source[@"profile"]?:@"",@"folder_checked":@YES};
 } @catch(NSException *exception) {return Failure(Problem(@"The selected folder could not be accessed. Your local save files are unchanged."));}
 @finally {atomic_store(&finished,true);}
}
void PeggleFolderExchange(NSDictionary *source,NSDictionary *(^merge)(NSDictionary *),void(^completion)(NSDictionary *)) {
 NSData *bookmark=[NSUserDefaults.standardUserDefaults dataForKey:BookmarkKey];NSString *context=PeggleSyncContext();
 if(!bookmark.length){completion(Failure(Problem(@"Choose an iCloud Drive folder in Manage Save Files before enabling sync.")));return;}
 dispatch_async(FolderQueue(),^{@autoreleasepool {
  NSError *error=nil;BOOL stale=NO;NSURL *folder=[NSURL URLByResolvingBookmarkData:bookmark options:NSURLBookmarkResolutionWithoutUI relativeToURL:nil bookmarkDataIsStale:&stale error:&error];
  BOOL scoped=folder&&[folder startAccessingSecurityScopedResource];NSDictionary *result=nil;NSData *fresh=nil;
  if(!scoped)result=Failure(Problem(@"The selected folder is no longer accessible. Choose it again in Manage Save Files. Your local save files are retained."));
  else {
   NSString *writer=WriterID(&error);
   if(stale)fresh=[folder bookmarkDataWithOptions:NSURLBookmarkCreationMinimalBookmark includingResourceValuesForKeys:nil relativeToURL:nil error:&error];
   result=error?Failure(error):PeggleFolderExchangeAtURL(folder,writer,source,merge);
   [folder stopAccessingSecurityScopedResource];
  }
  dispatch_async(dispatch_get_main_queue(),^{
   if(![context isEqual:PeggleSyncContext()]){completion(@{@"ok":@NO,@"cancelled":@YES});return;}
   if(fresh)[NSUserDefaults.standardUserDefaults setObject:fresh forKey:BookmarkKey];
   completion(result?:Failure(error));
  });
 }});
}
