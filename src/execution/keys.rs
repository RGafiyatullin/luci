use slotmap::new_key_type;

new_key_type! {
    pub struct KeyBind;
    pub struct KeySend;
    pub struct KeyRecv;
    pub struct KeyRespond;
    pub struct KeyDelay;
}

new_key_type! {
    pub struct KeyScope;
}

new_key_type! {
    pub struct KeyScenario;
}
