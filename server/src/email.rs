//! Deciding whether an address is worth accepting.
//!
//! An address here is the only way back to a player: their account, their coins and their invites hang off it.
//! A typo ("gmaill.com") or a throwaway ("test@test.com") means a player who cannot be reached and a seat that
//! cannot be recovered, so both are refused at sign-up rather than discovered later. We never send mail from here,
//! so this is all the checking there is.

/// Why an address was refused, in words a player can act on.
pub type Rejection = String;

const MAX_LEN: usize = 254;
const MAX_LOCAL: usize = 64;

/// Local-part characters RFC 5322 allows unquoted. Quoted local parts ("a b"@x.com) are legal and refused anyway:
/// no real sign-up needs them and they break every downstream tool.
const LOCAL_OK: &str = "abcdefghijklmnopqrstuvwxyz0123456789.!#$%&'*+/=?^_`{|}~-";

/// Domains that are never a real inbox: documentation examples, local names and obvious stand-ins.
const FAKE: &[&str] = &[
    "test.com", "test.test", "test.org", "test.net", "example.com", "example.org", "example.net", "example.edu",
    "domain.com", "email.com", "mail.com", "sample.com", "demo.com", "fake.com", "none.com", "no.com", "abc.com",
    "asdf.com", "qwerty.com", "localhost", "localhost.com", "invalid", "guest.invalid", "nowhere.com", "xyz.com",
    "a.com", "aa.com", "123.com", "yourdomain.com", "yoursite.com", "company.com", "gmail.con",
];

/// Throwaway inbox services. Short list on purpose: it covers the ones people actually reach for, and anything
/// missed is still a working address, not a broken one.
const DISPOSABLE: &[&str] = &[
    "mailinator.com", "10minutemail.com", "guerrillamail.com", "guerrillamail.net", "sharklasers.com",
    "yopmail.com", "yopmail.net", "temp-mail.org", "tempmail.com", "tempmail.net", "throwawaymail.com",
    "trashmail.com", "getnada.com", "nada.email", "maildrop.cc", "dispostable.com", "fakeinbox.com",
    "mailnesia.com", "mohmal.com", "emailondeck.com", "spamgourmet.com", "mytemp.email", "tmpmail.org",
    "moakt.com", "inboxkitten.com", "burnermail.io", "33mail.com", "grr.la", "spam4.me", "discard.email",
];

/// Near misses of the big providers, with what was almost certainly meant. The message quotes the fix, so the
/// player corrects it in one tap instead of guessing.
const TYPOS: &[(&str, &str)] = &[
    ("gmaill.com", "gmail.com"), ("gmai.com", "gmail.com"), ("gmial.com", "gmail.com"), ("gmal.com", "gmail.com"),
    ("gmail.co", "gmail.com"), ("gmail.cm", "gmail.com"), ("gmail.om", "gmail.com"), ("gmail.vom", "gmail.com"),
    ("gmaii.com", "gmail.com"), ("gnail.com", "gmail.com"), ("gmil.com", "gmail.com"), ("gamil.com", "gmail.com"),
    ("gmail.comm", "gmail.com"), ("gmailc.om", "gmail.com"), ("ggmail.com", "gmail.com"), ("gmaik.com", "gmail.com"),
    ("yahooo.com", "yahoo.com"), ("yaho.com", "yahoo.com"), ("yahoo.co", "yahoo.com"), ("yahho.com", "yahoo.com"),
    ("yhoo.com", "yahoo.com"), ("yahoo.cm", "yahoo.com"),
    ("hotmial.com", "hotmail.com"), ("hotmai.com", "hotmail.com"), ("hotmail.co", "hotmail.com"),
    ("hotmaill.com", "hotmail.com"), ("hotmall.com", "hotmail.com"), ("homail.com", "hotmail.com"),
    ("outlok.com", "outlook.com"), ("outllook.com", "outlook.com"), ("outlook.co", "outlook.com"),
    ("outook.com", "outlook.com"), ("oulook.com", "outlook.com"),
    ("iclou.com", "icloud.com"), ("icloud.co", "icloud.com"), ("iclod.com", "icloud.com"),
    ("protonmai.com", "protonmail.com"), ("protonmail.co", "protonmail.com"),
];

/// Checks an address and returns it trimmed and lower-cased, or the reason it cannot be used.
pub fn check(raw: &str) -> Result<String, Rejection> {
    let e = raw.trim().to_lowercase();

    if e.is_empty() {
        return Err("ইমেইল দিন".into());
    }
    if e.len() > MAX_LEN || e.contains(char::is_whitespace) {
        return Err("সঠিক ইমেইল দিন".into());
    }

    let mut parts = e.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = match parts.next() {
        Some(d) if !d.is_empty() => d,
        _ => return Err("ইমেইলে @ চিহ্ন থাকতে হবে, যেমন nam@gmail.com".into()),
    };
    if domain.contains('@') || local.is_empty() {
        return Err("সঠিক ইমেইল দিন".into());
    }

    // --- the part before @
    if local.len() > MAX_LOCAL {
        return Err("ইমেইলের @ এর আগের অংশ অনেক বড়".into());
    }
    if local.starts_with('.') || local.ends_with('.') || local.contains("..") {
        return Err("ইমেইলে পরপর দুটি ডট বা শুরু/শেষে ডট থাকতে পারে না".into());
    }
    if local.chars().any(|c| !LOCAL_OK.contains(c)) {
        return Err("ইমেইলে ইংরেজি অক্ষর, সংখ্যা আর . _ - চিহ্ন ব্যবহার করুন".into());
    }

    // --- the domain
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") || domain.starts_with('-') {
        return Err("ইমেইলের ডোমেইনটি সঠিক নয়".into());
    }
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return Err("ইমেইলের ডোমেইনে ডট থাকতে হবে, যেমন gmail.com".into());
    }
    for l in &labels {
        if l.is_empty() || l.len() > 63 || l.starts_with('-') || l.ends_with('-') {
            return Err("ইমেইলের ডোমেইনটি সঠিক নয়".into());
        }
        if l.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '-')) {
            return Err("ইমেইলের ডোমেইনটি সঠিক নয়".into());
        }
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || tld.chars().any(|c| !c.is_ascii_alphabetic()) {
        return Err("ইমেইলের শেষ অংশটি সঠিক নয়, যেমন .com বা .net".into());
    }

    // --- addresses that are valid but will never reach anyone
    if let Some((_, right)) = TYPOS.iter().find(|(wrong, _)| *wrong == domain) {
        return Err(format!("ডোমেইনে ভুল আছে। আপনি কি {local}@{right} বলতে চেয়েছেন?"));
    }
    if FAKE.contains(&domain) {
        return Err("আসল ইমেইল দিন, পরীক্ষার ঠিকানা নয়".into());
    }
    if DISPOSABLE.contains(&domain) {
        return Err("অস্থায়ী ইমেইল সেবা ব্যবহার করা যাবে না, নিজের ইমেইল দিন".into());
    }
    // Our own guest placeholders are built from this and must never be typed in by hand.
    if domain.ends_with("guest.invalid") {
        return Err("আসল ইমেইল দিন".into());
    }

    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_addresses_pass_and_come_back_tidy() {
        for raw in ["zia@gmail.com", "  Zia.Rana+kolom@Gmail.COM ", "a_b-c@sub.domain.co.uk", "x1@yahoo.com", "me@bracu.ac.bd"] {
            let got = check(raw).unwrap_or_else(|e| panic!("{raw} was refused: {e}"));
            assert_eq!(got, raw.trim().to_lowercase());
        }
    }

    #[test]
    fn the_two_the_shop_actually_saw_are_refused() {
        // Both of these reached the live sign-up form.
        let a = check("ahovatija@gmaill.com").unwrap_err();
        assert!(a.contains("gmail.com"), "it should offer the fix: {a}");
        assert!(check("test@test.com").is_err());
    }

    #[test]
    fn broken_shapes_are_refused() {
        for raw in [
            "", "   ", "nobody", "no@domain", "@gmail.com", "a@@gmail.com", "a b@gmail.com", "a@gmail .com",
            ".a@gmail.com", "a.@gmail.com", "a..b@gmail.com", "a@.gmail.com", "a@gmail..com", "a@gmail.com.",
            "a@-gmail.com", "a@gmail-.com", "a@gmail.c", "a@gmail.123", "নাম@gmail.com", "a@localhost",
        ] {
            assert!(check(raw).is_err(), "{raw:?} should have been refused");
        }
    }

    #[test]
    fn throwaway_and_stand_in_domains_are_refused() {
        for raw in ["x@mailinator.com", "x@yopmail.com", "x@10minutemail.com", "x@example.com", "x@localhost.com", "guest-abc@guest.invalid"] {
            assert!(check(raw).is_err(), "{raw} should have been refused");
        }
    }

    #[test]
    fn a_typo_names_the_address_it_means() {
        let msg = check("rana@gmial.com").unwrap_err();
        assert!(msg.contains("rana@gmail.com"), "{msg}");
    }

    #[test]
    fn length_limits_hold() {
        assert!(check(&format!("{}@gmail.com", "a".repeat(65))).is_err());
        assert!(check(&format!("{}@gmail.com", "a".repeat(64))).is_ok());
        assert!(check(&format!("{}@{}.com", "a".repeat(64), "b".repeat(200))).is_err());
    }
}
