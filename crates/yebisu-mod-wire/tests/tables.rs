//! Each reflection table moves exactly its own handshake digest.
use yebisu_mod_wire::*;

fn tables() -> ValidationTables {
    ValidationTables {
        styles: [(1u16, "flex".to_string())].into_iter().collect(),
        param_styles: ["w".to_string()].into_iter().collect(),
        methods: ["id".to_string()].into_iter().collect(),
        callbacks: ["on_click".to_string()].into_iter().collect(),
        actions: ["pane.split".to_string()].into_iter().collect(),
        slots: ["content".to_string()].into_iter().collect(),
    }
}

#[test]
fn equal_tables_have_equal_digests_and_each_table_moves_its_own() {
    let base = tables().digests().unwrap();
    assert_eq!(base, tables().digests().unwrap());
    let changed = |change: fn(&mut ValidationTables)| {
        let mut t = tables();
        change(&mut t);
        t.digests().unwrap()
    };
    let style = changed(|t| {
        t.styles.insert(1, "block".into());
    });
    assert_ne!(style.style, base.style);
    assert_eq!(
        (style.op, style.command),
        (base.op.clone(), base.command.clone())
    );
    let param = changed(|t| {
        t.param_styles.insert("h".into());
    });
    assert_ne!(param.style, base.style);
    for op in [
        changed(|t| {
            t.methods.insert("tooltip".into());
        }),
        changed(|t| {
            t.callbacks.insert("on_hover".into());
        }),
        changed(|t| {
            t.slots.insert("trigger".into());
        }),
    ] {
        assert_ne!(op.op, base.op);
        assert_eq!(op.style, base.style);
    }
    let command = changed(|t| {
        t.actions.insert("app.quit".into());
    });
    assert_ne!(command.command, base.command);
    assert_eq!(command.op, base.op);
    // A Hello carrying another side's digests is incompatible.
    let hello = Hello {
        client: "ipad".into(),
        nonce: Id128::new(format!("{:032x}", 1)).unwrap(),
        protocol: PROTOCOL,
        schema: SCHEMA.into(),
        digests: command,
        features: vec![],
        limits: NegotiatedLimits::default(),
    };
    assert!(hello.validate_compatibility(&base, &[]).is_err());
}
