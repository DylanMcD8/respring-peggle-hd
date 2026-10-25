#import <Foundation/Foundation.h>
#import "folder_sync.h"
#include <string.h>
extern char *peggle_save_manager_action(int,const char *,int);
extern void peggle_save_manager_free(char *);
static NSDictionary *Act(int code,NSString *value){char *s=peggle_save_manager_action(code,value.UTF8String,0);NSData *d=[NSData dataWithBytes:s length:strlen(s)];peggle_save_manager_free(s);return [NSJSONSerialization JSONObjectWithData:d options:0 error:NULL];}
int main(int argc,const char **argv){@autoreleasepool {
 if(argc!=5)return 2;NSDictionary *result=nil;
 if(strcmp(argv[1],"export")==0){result=Act(0,@(argv[2]));if([result[@"ok"]boolValue])result=Act(10,@" ");}
 else {NSDictionary *source=[NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfFile:@(argv[3])] options:0 error:NULL];result=PeggleFolderExchangeAtURL([NSURL fileURLWithPath:@(argv[1])],@(argv[2]),source,^NSDictionary *(NSDictionary *request){NSData *d=[NSJSONSerialization dataWithJSONObject:request options:0 error:NULL];return Act(20,[[NSString alloc]initWithData:d encoding:NSUTF8StringEncoding]);});}
 NSData *data=[NSJSONSerialization dataWithJSONObject:result options:NSJSONWritingSortedKeys error:NULL];[data writeToFile:@(argv[4]) atomically:YES];return 0;
}}
