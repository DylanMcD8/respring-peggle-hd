/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UITextField`.
//!
//! Useful resources:
//! - [UITextFieldDelegate overview](https://developer.apple.com/documentation/uikit/uitextfielddelegate?language=objc)

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::core_graphics::{CGFloat, CGPoint, CGRect, CGSize};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::frameworks::foundation::{ns_string, NSInteger, NSRange, NSUInteger};
use crate::frameworks::uikit::ui_font::{UITextAlignment, UITextAlignmentLeft};
use crate::frameworks::uikit::ui_view::ui_window::{
    UIKeyboardDidHideNotification, UIKeyboardDidShowNotification, UIKeyboardWillHideNotification,
    UIKeyboardWillShowNotification,
};
use crate::impl_HostObject_with_superclass;
use crate::objc::{
    id, msg, msg_class, msg_super, nil, objc_classes, release, todo_objc_setter, ClassExports,
    NSZonePtr, SEL,
};
use crate::Environment;

type UIKeyboardAppearance = NSInteger;
type UIKeyboardType = NSInteger;
pub type UIReturnKeyType = NSInteger;
type UITextAutocapitalizationType = NSInteger;
type UITextAutocorrectionType = NSInteger;

const UITextFieldTextDidChangeNotification: &str = "UITextFieldTextDidChangeNotification";

/// `NSNotificationName` values.
pub const CONSTANTS: ConstantExports = &[(
    "_UITextFieldTextDidChangeNotification",
    HostConstant::NSString(UITextFieldTextDidChangeNotification),
)];

struct UITextFieldHostObject {
    superclass: super::UIControlHostObject,
    delegate: id,
    editing: bool,
    text_label: id,
}
impl_HostObject_with_superclass!(UITextFieldHostObject);
impl Default for UITextFieldHostObject {
    fn default() -> Self {
        UITextFieldHostObject {
            superclass: Default::default(),
            delegate: nil,
            editing: false,
            text_label: nil,
        }
    }
}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UITextField: UIControl

// TODO: additional rendering (e.g. placeholder, border, clear button, etc.)
// TODO: more properties
// TODO: notifications

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UITextFieldHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    let this: id = msg_super![env; this initWithFrame:frame];

    () = msg![env; this setOpaque:true];
    let bg_color: id = msg_class![env; UIColor whiteColor];
    () = msg![env; this setBackgroundColor:bg_color];

    let text_label: id = msg_class![env; UILabel new];
    () = msg![env; text_label setBackgroundColor:bg_color];
    () = msg![env; text_label setTextAlignment:UITextAlignmentLeft];

    let text_color: id = msg_class![env; UIColor blackColor];
    () = msg![env; text_label setTextColor:text_color];

    let host_obj = env.objc.borrow_mut::<UITextFieldHostObject>(this);
    host_obj.text_label = text_label;

    () = msg![env; this addSubview:text_label];

    this
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder: coder];

    // TODO: actual decoding of properties

    let text_label: id = msg_class![env; UILabel new];

    let host_obj = env.objc.borrow_mut::<UITextFieldHostObject>(this);
    host_obj.text_label = text_label;

    () = msg![env; this addSubview:text_label];

    this
}

- (())dealloc {
    let UITextFieldHostObject {
        text_label,
        ..
    } = std::mem::take(env.objc.borrow_mut(this));

    release(env, text_label);
    msg_super![env; this dealloc]
}

- (())layoutSubviews {
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    let bounds: CGRect = msg![env; this bounds];

    () = msg![env; text_label setFrame:bounds];
}

- (id)text {
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    msg![env; text_label text]
}
- (())setText:(id)text { // NSString*
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    () = msg![env; text_label setText:text];

    // This will work only if all the text changes will call setText:!
    // This is the case right now.
    // (see `handle_text` and `handle_backspace` helper functions below)
    // TODO: actually check if setText: send this notif on each change
    // (e.g. does it send the notif if text hasn't changed)
    let center: id = msg_class![env; NSNotificationCenter defaultCenter];
    let name = ns_string::get_static_str(env, UITextFieldTextDidChangeNotification);
    // TODO: userInfo
    keyboard_trace(env,crate::keyboard_trace::Stage::NotificationBegin,this,nil,0,0,0,0);
    let _: () = msg![env; center postNotificationName:name object:this userInfo:nil];
    keyboard_trace(env,crate::keyboard_trace::Stage::NotificationReturned,this,nil,0,0,0,0);
}

- (())setTextColor:(id)color { // UIColor*
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    msg![env; text_label setTextColor:color]
}

- (())setTextAlignment:(UITextAlignment)text_alignment {
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    () = msg![env; text_label setTextAlignment:text_alignment];
}

- (())setFont:(id)new_font { // UIFont*
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    msg![env; text_label setFont:new_font]
}

- (())setMinimumFontSize:(CGFloat)size {
    let text_label = env.objc.borrow_mut::<UITextFieldHostObject>(this).text_label;
    () = msg![env; text_label setMinimumFontSize:size];
}

- (())setClearsOnBeginEditing:(bool)clear {
    todo_objc_setter!(this, clear);
}

- (())setClearButtonMode:(NSInteger)mode {
    todo_objc_setter!(this, mode);
}

- (())setSecureTextEntry:(bool)secure {
    todo_objc_setter!(this, secure);
}

- (())setPlaceholder:(id)placeholder { // NSString*
    todo_objc_setter!(this, to_rust_string(env, placeholder));
}

- (())setPosition:(CGPoint)position {
    todo_objc_setter!(this, position);
}

// weak/non-retaining
- (())setDelegate:(id)delegate { // something implementing UITextFieldDelegate
    log_dbg!("setDelegate:{:?}", delegate);
    let host_object = env.objc.borrow_mut::<UITextFieldHostObject>(this);
    host_object.delegate = delegate;
}
- (id)delegate {
    env.objc.borrow::<UITextFieldHostObject>(this).delegate
}

// UITextInputTraits implementation
- (())setAutocapitalizationType:(UITextAutocapitalizationType)type_ {
    todo_objc_setter!(this, type_);
}
- (())setAutocorrectionType:(UITextAutocorrectionType)type_ {
    todo_objc_setter!(this, type_);
}
- (())setReturnKeyType:(UIReturnKeyType)type_ {
    todo_objc_setter!(this, type_);
}
- (())setKeyboardAppearance:(UIKeyboardAppearance)appearance {
    todo_objc_setter!(this, appearance);
}
- (())setKeyboardType:(UIKeyboardType)type_ {
    todo_objc_setter!(this, type_);
}
- (())setBorderStyle:(NSInteger)style {
    todo_objc_setter!(this, style);
}
- (())setEnablesReturnKeyAutomatically:(bool)enables {
    todo_objc_setter!(this, enables);
}

- (())touchesBegan:(id)_touches // NSSet* of UITouch*
         withEvent:(id)_event { // UIEvent*
    let _: bool = msg![env; this becomeFirstResponder];
}

- (bool)isEditing {
    env.objc.borrow::<UITextFieldHostObject>(this).editing
}

- (bool)becomeFirstResponder {
    log_dbg!("becomeFirstResponder");

    if env.objc.borrow::<UITextFieldHostObject>(this).editing {
        return true;
    }

    let delegate: id = env.objc.borrow::<UITextFieldHostObject>(this).delegate;
    let sel: SEL = env.objc.register_host_selector("textFieldShouldBeginEditing:".to_string(), &mut env.mem);
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if delegate != nil && responds && !msg![env; delegate textFieldShouldBeginEditing:this] {
        return false;
    }

    // If text is nil, it becomes an empty string
    // on becoming the first responder.
    // This behaviour was validated on the Aspen Simulator
    let text_label = env
        .objc
        .borrow_mut::<UITextFieldHostObject>(this)
        .text_label;
    let curr_text: id = msg![env; text_label text];
    if curr_text == nil {
        let empty = ns_string::get_static_str(env, "");
        () = msg![env; text_label setText:empty];
    }

    if !cfg!(target_os="ios") { post_keyboard_notification(env,1,[0.0;4],[0.0;4],[0.0;4],0.0,0); }

    env.framework_state.uikit.ui_responder.first_responder = this;
    env.on_parent_stack_in_coroutine(|window, _| window.start_text_input());

    if !cfg!(target_os="ios") { post_keyboard_notification(env,2,[0.0;4],[0.0;4],[0.0;4],0.0,0); }

    // TODO: is it the right spot?
    env.objc.borrow_mut::<UITextFieldHostObject>(this).editing = true;

    let sel: SEL = env.objc.register_host_selector("textFieldDidBeginEditing:".to_string(), &mut env.mem);
    if msg![env; delegate respondsToSelector:sel] {
        () = msg![env; delegate textFieldDidBeginEditing:this];
    }

    true
}

- (bool)resignFirstResponder {
    log_dbg!("resignFirstResponder");

    if !env.objc.borrow::<UITextFieldHostObject>(this).editing {
        return true;
    }

    let delegate: id = env.objc.borrow::<UITextFieldHostObject>(this).delegate;
    let sel: SEL = env.objc.register_host_selector("textFieldShouldEndEditing:".to_string(), &mut env.mem);
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    if delegate != nil && responds && !msg![env; delegate textFieldShouldEndEditing:this] {
        return false;
    }

    if !cfg!(target_os="ios") { post_keyboard_notification(env,3,[0.0;4],[0.0;4],[0.0;4],0.0,0); }

    env.framework_state.uikit.ui_responder.first_responder = nil;
    env.on_parent_stack_in_coroutine(|window, _| window.stop_text_input());

    if !cfg!(target_os="ios") { post_keyboard_notification(env,4,[0.0;4],[0.0;4],[0.0;4],0.0,0); }

    // TODO: is it the right spot?
    env.objc.borrow_mut::<UITextFieldHostObject>(this).editing = false;

    let sel: SEL = env.objc.register_host_selector("textFieldDidEndEditing:".to_string(), &mut env.mem);
    if msg![env; delegate respondsToSelector:sel] {
        () = msg![env; delegate textFieldDidEndEditing:this];
    }

    true
}

@end

};

/// Native notifications supply initialized rectangles and real animation
/// metadata. Peggle reads the legacy bounds through NSValue::getValue:.
pub fn post_keyboard_notification(env:&mut Environment, phase:i32, begin:[f32;4], end:[f32;4], bounds:[f32;4], duration:f64, curve:i32) {
    let name=match phase { 1=>UIKeyboardWillShowNotification,2=>UIKeyboardDidShowNotification,3=>UIKeyboardWillHideNotification,4=>UIKeyboardDidHideNotification,_=>return };
    fn rect(a:[f32;4])->CGRect { CGRect { origin:CGPoint{x:a[0],y:a[1]},size:CGSize{width:a[2],height:a[3]} } }
    let mut values=Vec::with_capacity(5);
    for (key,value) in [("UIKeyboardBoundsUserInfoKey",bounds),("UIKeyboardFrameBeginUserInfoKey",begin),("UIKeyboardFrameEndUserInfoKey",end)] {
        let key=ns_string::get_static_str(env,key);
        let value:id=msg_class![env; NSValue valueWithCGRect:(rect(value))];
        values.push((key,value));
    }
    let key=ns_string::get_static_str(env,"UIKeyboardAnimationDurationUserInfoKey");
    let value:id=msg_class![env; NSNumber numberWithDouble:duration]; values.push((key,value));
    let key=ns_string::get_static_str(env,"UIKeyboardAnimationCurveUserInfoKey");
    let value:id=msg_class![env; NSNumber numberWithInt:curve]; values.push((key,value));
    let info=crate::frameworks::foundation::ns_dictionary::dict_from_keys_and_objects(env,&values);
    let name_id=ns_string::get_static_str(env,name);
    let center:id=msg_class![env; NSNotificationCenter defaultCenter];
    crate::diagnostics::event(&format!("{{\"event\":\"guest_keyboard\",\"phase\":{phase},\"bounds\":{bounds:?},\"begin\":{begin:?},\"end\":{end:?},\"duration_s\":{duration},\"curve\":{curve}}}"));
    let _:()=msg![env; center postNotificationName:name_id object:nil userInfo:info];
    release(env,info);
}

pub fn handle_text(env: &mut Environment, text_field: id, text: String) {
    let replacement_units=text.encode_utf16().count()as u32;
    keyboard_trace(env,crate::keyboard_trace::Stage::InsertBegin,text_field,nil,0,replacement_units,0,0);
    // SDL commits can contain a word, pasted text, or a surrogate pair.
    // Pass the complete replacement to the original delegate as one edit.
    if text.is_empty() { return; }
    let txt = ns_string::from_rust_string(env, text);
    keyboard_trace(env,crate::keyboard_trace::Stage::ReplacementAllocated,text_field,nil,0,replacement_units,0,0);

    let text_label = env
        .objc
        .borrow_mut::<UITextFieldHostObject>(text_field)
        .text_label;
    let mut curr_text = msg![env; text_label text];
    if curr_text == nil {
        curr_text = ns_string::get_static_str(env, "");
    }
    log_dbg!(
        "handle_text, curr_text: {}",
        ns_string::to_rust_string(env, curr_text)
    );

    let len = msg![env; curr_text length];
    keyboard_trace(env,crate::keyboard_trace::Stage::CurrentTextRead,text_field,nil,len,replacement_units,len,0);
    let range = NSRange {
        location: len,
        length: 0,
    };

    let delegate: id = env
        .objc
        .borrow::<UITextFieldHostObject>(text_field)
        .delegate;
    let sel: SEL = env.objc.register_host_selector(
        "textField:shouldChangeCharactersInRange:replacementString:".to_string(),
        &mut env.mem,
    );
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    keyboard_trace(env,crate::keyboard_trace::Stage::DelegateBegin,text_field,delegate,len,replacement_units,len,0);
    let should = delegate == nil
        || !responds
        || msg![env; delegate textField:text_field shouldChangeCharactersInRange:range replacementString:txt];
    keyboard_trace(env,crate::keyboard_trace::Stage::DelegateReturned,text_field,delegate,len,replacement_units,len,u32::from(should));
    if should {
        keyboard_trace(env,crate::keyboard_trace::Stage::AppendBegin,text_field,delegate,len,replacement_units,len,0);
        let new_text: id = msg![env; curr_text stringByAppendingString:txt];
        keyboard_trace(env,crate::keyboard_trace::Stage::AppendReturned,text_field,delegate,len,replacement_units,len,0);
        log_dbg!(
            "handle_text, new_text: {}",
            ns_string::to_rust_string(env, new_text)
        );
        // TODO: refactor this to proper update() method
        keyboard_trace(env,crate::keyboard_trace::Stage::SetTextBegin,text_field,delegate,len,replacement_units,len,0);
        () = msg![env; text_field setText:new_text];
        keyboard_trace(env,crate::keyboard_trace::Stage::SetTextReturned,text_field,delegate,len,replacement_units,len,0);
        () = msg![env; text_field setNeedsDisplay];
        keyboard_trace(env,crate::keyboard_trace::Stage::DisplayInvalidated,text_field,delegate,len,replacement_units,len,0);
        release(env, new_text);
    }
    release(env, txt);
    keyboard_trace(env,crate::keyboard_trace::Stage::InsertComplete,text_field,delegate,len,replacement_units,len,0);
}

pub fn handle_backspace(env: &mut Environment, text_field: id) {
    keyboard_trace(env,crate::keyboard_trace::Stage::BackspaceBegin,text_field,nil,0,0,0,0);
    let text_label = env
        .objc
        .borrow_mut::<UITextFieldHostObject>(text_field)
        .text_label;
    let curr_text: id = msg![env; text_label text];

    let len: NSUInteger = msg![env; curr_text length];
    if len == 0 {
        return;
    }
    let range = {
        let text = ns_string::to_rust_string(env, curr_text);
        let Some((location, length)) = crate::keyboard_text::trailing_scalar_range(&text) else { return; };
        NSRange { location, length }
    };
    let deletion_start = range.location;
    let empty = ns_string::get_static_str(env, "");

    let delegate: id = env
        .objc
        .borrow::<UITextFieldHostObject>(text_field)
        .delegate;
    let sel: SEL = env.objc.register_host_selector(
        "textField:shouldChangeCharactersInRange:replacementString:".to_string(),
        &mut env.mem,
    );
    let responds: bool = msg![env; delegate respondsToSelector:sel];
    let should = delegate == nil
        || !responds
        || msg![env; delegate textField:text_field shouldChangeCharactersInRange:range replacementString:empty];
    if should {
        let new_text: id = msg![env; curr_text substringToIndex:deletion_start];
        log_dbg!(
            "handle_backspace, new_text: {}",
            ns_string::to_rust_string(env, new_text)
        );
        // TODO: refactor this to proper update() method
        () = msg![env; text_field setText:new_text];
        () = msg![env; text_field setNeedsDisplay];
        release(env, new_text);
    }
}

pub fn handle_return(env: &mut Environment, text_field: id) {
    keyboard_trace(env,crate::keyboard_trace::Stage::ReturnBegin,text_field,nil,0,0,0,0);
    let delegate: id = env
        .objc
        .borrow::<UITextFieldHostObject>(text_field)
        .delegate;
    let sel: SEL = env
        .objc
        .register_host_selector("textFieldShouldReturn:".to_string(), &mut env.mem);
    if msg![env; delegate respondsToSelector:sel] {
        log_dbg!("handle_return");
        () = msg![env; delegate textFieldShouldReturn:text_field];
    }
}

fn keyboard_trace(env:&Environment,stage:crate::keyboard_trace::Stage,field:id,delegate:id,current_units:u32,replacement_units:u32,location:u32,aux:u32){crate::keyboard_trace::record(stage,crate::keyboard_trace::Meta{field:field.to_bits(),delegate:delegate.to_bits(),current_units,replacement_units,location,aux,thread:env.current_thread as u32,pc:env.cpu.regs()[15],lr:env.cpu.regs()[14],sp:env.cpu.regs()[13]});}
