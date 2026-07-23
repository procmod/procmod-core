use procmod_core::{Address, Architecture, PeSection};

#[test]
fn immutable_sections_are_read_only() {
    let text = PeSection {
        name: ".text".into(),
        address: Address::new(0x1000),
        virtual_size: 0x100,
        mapped_size: 0x200,
        readable: true,
        writable: false,
        executable: true,
    };
    assert!(text.immutable());

    let mut data = text;
    data.writable = true;
    assert!(!data.immutable());
}

#[test]
fn x86_section_ranges_are_bounded() {
    let start = Address::new(0xffff_f000);
    assert!(start.validate_for(Architecture::X86).is_ok());
    assert!(start
        .checked_add(0x1000)
        .unwrap()
        .validate_for(Architecture::X86)
        .is_err());
}
