#import <Foundation/Foundation.h>
#import <dispatch/dispatch.h>
// Tokens belong only to this picker selection and are released once after the
// coordinated read, including parser/coordination errors. No bookmarks persist.
static inline dispatch_block_t PeggleAcquireImportScopes(NSArray<NSURL *> *urls) {
 if(urls.count==0||urls.count>128)return nil;
 for(NSURL *url in urls)if(!url.isFileURL)return nil;
 NSMutableArray<NSURL *> *started=[NSMutableArray new];
 @try{for(NSURL *url in urls)if([url startAccessingSecurityScopedResource])[started addObject:url];}
 @catch(NSException *exception){for(NSURL *url in started)[url stopAccessingSecurityScopedResource];@throw;}
 NSArray<NSURL *> *held=[started copy];NSLock *lock=[NSLock new];__block BOOL released=NO;
 return [^{[lock lock];BOOL first=!released;released=YES;[lock unlock];if(first)for(NSURL *url in held)[url stopAccessingSecurityScopedResource];} copy];
}
typedef NSDictionary *(^PeggleImportRead)(NSArray<NSString *> *paths);
static NSDictionary *PeggleCoordinateImportUsing(NSFileCoordinator *coordinator,NSArray<NSURL *> *urls,NSUInteger index,NSArray<NSString *> *paths,PeggleImportRead operation){
 if(index==urls.count)return operation(paths);
 __block NSDictionary *result=nil;NSError *error=nil;
 [coordinator coordinateReadingItemAtURL:urls[index] options:NSFileCoordinatorReadingWithoutChanges error:&error byAccessor:^(NSURL *url){
  result=PeggleCoordinateImportUsing(coordinator,urls,index+1,[paths arrayByAddingObject:url.path],operation);
 }];
 if(error)return @{@"ok":@NO,@"error":@"The selected provider could not make these files available. Try downloading them in Files first."};
 return result?:@{@"ok":@NO,@"error":@"The selected files could not be read."};
}

static inline NSDictionary *PeggleCoordinateImport(NSArray<NSURL *> *urls,NSUInteger index,NSArray<NSString *> *paths,PeggleImportRead operation){return PeggleCoordinateImportUsing([[NSFileCoordinator alloc]initWithFilePresenter:nil],urls,index,paths,operation);}
