error_chain! {
    foreign_links {
        Diesel(::diesel::result::Error);
        R2d2GetTimeout(::r2d2::GetTimeout);
        Io(::std::io::Error);
    }
}