// Copyright (c) Facebook, Inc. and its affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the "hack" directory of this source tree.

use std::borrow::Cow;

/**
 * A\B\C -> \A\B\C
 */
pub fn add_ns(s: &str) -> Cow<str> {
    if !s.starts_with('\\') {
        let mut new_str = String::with_capacity(1 + s.len());
        new_str.push_str("\\");
        new_str.push_str(s);
        Cow::Owned(new_str)
    } else {
        Cow::Borrowed(s)
    }
}

/**
 * \A\B\C -> A\B\C
 */
pub fn strip_ns(s: &str) -> &str {
    if s.is_empty() || !s.starts_with('\\') {
        s
    } else {
        &s[1..]
    }
}

#[cfg(test)]
mod utils_tests {
    use pretty_assertions::assert_eq;

    #[test]
    fn add_ns_test() {
        let test_string = "\\MyTestClass";
        assert_eq!(super::add_ns(&test_string), "\\MyTestClass");

        let test_string2 = "MyTestClass";
        assert_eq!(super::add_ns(&test_string2), "\\MyTestClass");

        let test_string3 = "SubNamespace\\MyTestClass";
        assert_eq!(super::add_ns(&test_string3), "\\SubNamespace\\MyTestClass");

        let test_string4 = "\\SubNamespace\\MyTestClass";
        assert_eq!(super::add_ns(&test_string4), "\\SubNamespace\\MyTestClass");

        let test_string5 = "";
        assert_eq!(super::add_ns(&test_string5), "\\");
    }

    #[test]
    fn strip_ns_test() {
        let test_string = "\\MyTestClass";
        assert_eq!(super::strip_ns(&test_string), "MyTestClass");

        let test_string2 = "MyTestClass";
        assert_eq!(super::strip_ns(&test_string2), "MyTestClass");

        let test_string3 = "SubNamespace\\MyTestClass";
        assert_eq!(super::strip_ns(&test_string3), "SubNamespace\\MyTestClass");

        let test_string4 = "\\SubNamespace\\MyTestClass";
        assert_eq!(super::strip_ns(&test_string4), "SubNamespace\\MyTestClass");

        let test_string5 = "";
        assert_eq!(super::strip_ns(&test_string5), "");
    }
}
