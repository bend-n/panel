#[macro_export]
macro_rules! define_print {
    ($name:ident, $start:expr) => {
        paste::paste!(
            #[allow(unused_macros)]
            macro_rules! $name {
                () => {{
                    // #[cfg(debug_assertions)]
                    println!($start)
                }};
                ($fmt:literal) => {{
                    // #[cfg(debug_assertions)]
                    println!(concat!($start, " {}"), format!($fmt))
                }};
            }
        );
    };
    ($name:ident, $start:expr, $b:expr) => {
        paste::paste!(
            #[allow(unused_macros)]
            macro_rules! $name {
                () => {{
                    // #[cfg(debug_assertions)]
                    println!($start);
                    $b
                }};
                ($fmt:literal) => {{
                    // #[cfg(debug_assertions)]
                    println!(concat!($start, " {}"), format!($fmt));
                    $b
                }};
            }
        );
    };
    ($prefix:expr) => {
        define_print!(fail, concat!($prefix, " ", "!!"), break);
        define_print!(flush, concat!($prefix, " ", "<<"));
        define_print!(nooutput, concat!($prefix, " ", "!>"));
        define_print!(input, concat!($prefix, " ", "<"));
        define_print!(output, concat!($prefix, " ", ">"));
    };
}

#[macro_export]
macro_rules! dummy_print {
    ($name:ident, $start:expr) => {
        paste::paste!(
            #[allow(unused_macros)]
            macro_rules! $name {
                () => {
                    ()
                };
                ($fmt:literal) => {
                    ()
                };
            }
        );
    };
    ($name:ident, $start:expr, $b:expr) => {
        paste::paste!(
            #[allow(unused_macros)]
            macro_rules! $name {
                () => {{
                    $b
                }};
                ($fmt:literal) => {{
                    $b
                }};
            }
        );
    };
    ($prefix:expr) => {
        dummy_print!(fail, concat!($prefix, " ", "!!"), break);
        dummy_print!(flush, concat!($prefix, " ", "<<"));
        dummy_print!(nooutput, concat!($prefix, " ", "!>"));
        dummy_print!(input, concat!($prefix, " ", "<"));
        dummy_print!(output, concat!($prefix, " ", ">"));
    };
}
