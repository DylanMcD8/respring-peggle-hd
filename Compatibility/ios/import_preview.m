#import "import_preview.h"
#include <math.h>
@implementation PeggleRawImportPreview
- (void)viewDidLoad {
 [super viewDidLoad];self.title=@"Review Save Import";self.tableView.rowHeight=UITableViewAutomaticDimension;self.tableView.estimatedRowHeight=72;
 self.navigationItem.leftBarButtonItem=[[UIBarButtonItem alloc]initWithBarButtonSystemItem:UIBarButtonSystemItemCancel target:self action:@selector(cancel)];
 NSString *title=[NSString stringWithFormat:@"Import %@ Files",self.preview[@"files_to_import"]];
 self.navigationItem.rightBarButtonItem=[[UIBarButtonItem alloc]initWithTitle:title style:UIBarButtonItemStyleDone target:self action:@selector(confirm)];self.navigationItem.rightBarButtonItem.enabled=[self.preview[@"can_import"]boolValue];
 UILabel *footer=[UILabel new];footer.numberOfLines=0;footer.text=@"Only the listed files will be imported into a new set of save files. Other current files will be kept, and the previous set will remain available to restore. File checks cannot confirm where these save files came from or whether they load correctly in the game. Close the original game before selecting its save files.";footer.font=[UIFont preferredFontForTextStyle:UIFontTextStyleFootnote];footer.adjustsFontForContentSizeCategory=YES;footer.frame=CGRectMake(20,0,520,180);self.tableView.tableFooterView=footer;
}
- (void)viewDidLayoutSubviews {
 [super viewDidLayoutSubviews];UILabel *footer=(UILabel *)self.tableView.tableFooterView;CGFloat width=CGRectGetWidth(self.tableView.bounds);CGFloat height=[footer sizeThatFits:CGSizeMake(MAX(160,width),CGFLOAT_MAX)].height+32;if(fabs(CGRectGetHeight(footer.frame)-height)>1){footer.frame=CGRectMake(0,0,width,height);self.tableView.tableFooterView=footer;}
}
- (NSArray *)keys{return @[@"accepted",@"skipped",@"conflicts",@"missing_dependencies",@"notes"];}
- (NSInteger)numberOfSectionsInTableView:(UITableView *)tableView{return 5;}
- (NSInteger)tableView:(UITableView *)tableView numberOfRowsInSection:(NSInteger)section{return [self.preview[[self keys][section]]count];}
- (NSString *)tableView:(UITableView *)tableView titleForHeaderInSection:(NSInteger)section {
 NSString *name=@[@"Will Import",@"Skipped",@"Conflicts — Not Imported",@"Missing Dependencies — Not Imported",@"Selection and Compatibility"][section];return [NSString stringWithFormat:@"%@ (%lu)",name,(unsigned long)[self.preview[[self keys][section]]count]];
}
- (UITableViewCell *)tableView:(UITableView *)tableView cellForRowAtIndexPath:(NSIndexPath *)indexPath {
 UITableViewCell *cell=[tableView dequeueReusableCellWithIdentifier:@"row"];if(!cell)cell=[[UITableViewCell alloc]initWithStyle:UITableViewCellStyleSubtitle reuseIdentifier:@"row"];
 id row=self.preview[[self keys][indexPath.section]][indexPath.row];cell.selectionStyle=UITableViewCellSelectionStyleNone;
 if([row isKindOfClass:NSString.class]){cell.textLabel.text=row;cell.detailTextLabel.text=nil;}
 else{cell.textLabel.text=row[@"path"];cell.detailTextLabel.text=row[@"reason"]?:[NSString stringWithFormat:@"%@ bytes · %@\n%@",row[@"bytes"],row[@"action"],row[@"validation"]];}
 cell.textLabel.font=[UIFont preferredFontForTextStyle:UIFontTextStyleBody];cell.detailTextLabel.font=[UIFont preferredFontForTextStyle:UIFontTextStyleFootnote];cell.textLabel.numberOfLines=0;cell.detailTextLabel.numberOfLines=0;cell.textLabel.adjustsFontForContentSizeCategory=YES;cell.detailTextLabel.adjustsFontForContentSizeCategory=YES;return cell;
}
- (void)cancel {if(self.cancelImport)self.cancelImport();}
- (void)confirm {
 UIAlertController *alert=[UIAlertController alertControllerWithTitle:@"Import Reviewed Files?" message:@"Only files from the Will Import list will be imported." preferredStyle:UIAlertControllerStyleAlert];
 [alert addAction:[UIAlertAction actionWithTitle:@"Cancel" style:UIAlertActionStyleCancel handler:nil]];
 [alert addAction:[UIAlertAction actionWithTitle:@"Import Files" style:UIAlertActionStyleDestructive handler:^(UIAlertAction *action){self.navigationItem.rightBarButtonItem.enabled=NO;if(self.confirmImport)self.confirmImport();}]];[self presentViewController:alert animated:YES completion:nil];
}
@end
