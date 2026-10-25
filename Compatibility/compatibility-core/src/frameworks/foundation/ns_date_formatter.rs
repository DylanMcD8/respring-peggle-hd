/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSDateFormatter`.
//!
//! Resources:
//! - Apple's [Introduction to Data Formatting Programming Guide For Cocoa](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DataFormatting/DataFormatting.html)
//! - [Unicode Technical Standard #35](https://unicode.org/reports/tr35/tr35-10.html#Date_Format_Patterns)

use crate::frameworks::core_foundation::time::CFAbsoluteTimeGetGregorianDate;
use crate::frameworks::foundation::{ns_string, NSTimeInterval};
use crate::objc::{autorelease, id, msg, nil, objc_classes, release, retain, ClassExports, HostObject, NSZonePtr};

struct NSDateFormatterHostObject {
    date_format: Option<id>,
    locale: id,
}
impl HostObject for NSDateFormatterHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSDateFormatter: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(NSDateFormatterHostObject {
        date_format: None,
        locale: nil,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (())setDateFormat:(id)format { // NSString *
    let date_format: id = msg![env; format copy];
    let previous = env.objc.borrow_mut::<NSDateFormatterHostObject>(this).date_format.replace(date_format);
    if let Some(previous) = previous { release(env, previous); }
}

- (())setLocale:(id)locale {
    let locale = retain(env, locale);
    let previous = std::mem::replace(&mut env.objc.borrow_mut::<NSDateFormatterHostObject>(this).locale, locale);
    release(env, previous);
}

- (id)locale {
    env.objc.borrow::<NSDateFormatterHostObject>(this).locale
}

- (())dealloc {
    let &NSDateFormatterHostObject { date_format, locale } = env.objc.borrow(this);
    if let Some(format) = date_format { release(env, format); }
    release(env, locale);
    env.objc.dealloc_object(this, &mut env.mem);
}

- (id)stringFromDate:(id)date {
    let &NSDateFormatterHostObject {
        date_format, ..
    } = env.objc.borrow(this);
    let mut format = ns_string::to_rust_string(env, date_format.unwrap()).to_string().clone();
    log_dbg!("date_format before: {:?}", format);

    let ti: NSTimeInterval = msg![env; date timeIntervalSinceReferenceDate];
    let greg_date = CFAbsoluteTimeGetGregorianDate(env, ti, nil);
    let year = greg_date.year;
    let month = greg_date.month;
    let day = greg_date.day;
    let hour = greg_date.hours;
    let minute = greg_date.minutes;
    let second = greg_date.seconds;

    format = format.replace("yyyy", format!("{year:04}").as_str());
    format = format.replace("YYYY", format!("{year:04}").as_str());
    format = format.replace("MM", format!("{month:02}").as_str());
    format = format.replace("dd", format!("{day:02}").as_str());
    format = format.replace("HH", format!("{hour:02}").as_str());
    format = format.replace("mm", format!("{minute:02}").as_str());
    format = format.replace("ss", format!("{:02}", second.floor() as u32).as_str());
    format = fractional_seconds(&format, ti);

    for c in format.chars() {
        if let pattern @ ('A'..='Z' | 'a'..='z') = c {
            unimplemented!("date string contains unsubstituted format pattern: {pattern}");
        }
    }
    log_dbg!("date_format after: {:?}", format);

    let res = ns_string::from_rust_string(env, format);
    autorelease(env, res)
}

@end

};

fn fractional_seconds(format: &str, time_interval: f64) -> String {
    let digits = format!("{:09}", (time_interval.rem_euclid(1.0) * 1_000_000_000.0).floor() as u32);
    let mut chars = format.chars().peekable();
    let mut output = String::new();
    while let Some(c) = chars.next() {
        if c != 'S' { output.push(c); continue; }
        let mut width = 1;
        while chars.peek() == Some(&'S') { chars.next(); width += 1; }
        output.push_str(&digits[..width.min(9)]);
        for _ in 9..width { output.push('0'); }
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn fractional_precision_and_negative_reference_time() {
        assert_eq!(super::fractional_seconds("S/SS/SSS", 12.125), "1/12/125");
        assert_eq!(super::fractional_seconds("SSS", -0.75), "250");
        assert_eq!(super::fractional_seconds("SSSSSSSSSS", 0.5), "5000000000");
    }
}
