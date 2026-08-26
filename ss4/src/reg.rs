//! Модель регистра и базовые предикаты типа данных (bit/32-bit).

#[derive(Clone, Debug)]
/// Описание регистра из таблицы `reg` и его типа представления.
pub struct Reg {
    pub id: i32,
    #[allow(dead_code)]
    pub name: String,
    pub addr: i32,
    pub n_mb: Option<i32>,
    pub tip: i32,
    pub bits: Option<i32>,
    pub grup: Option<i32>,
    pub a_en: bool,
    pub a_no_write: i32,
}

impl Reg {
    /// Проверяет, что регистр занимает два 16-битных слова (tip=2/4/5).
    ///
    /// # Returns
    /// - `true`, если регистр 32-битный.
    /// - `false`, если регистр 16-битный или битовый.
    pub fn is_32(&self) -> bool {
        matches!(self.tip, 2 | 4 | 5)
    }

    /// Проверяет, что регистр является битовым полем (tip=0 и задан `bits`).
    ///
    /// # Returns
    /// - `true`, если регистр читается как отдельный бит.
    /// - `false` в остальных случаях.
    pub fn is_bit(&self) -> bool {
        self.tip == 0 && self.bits.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::Reg;

    fn mk_reg(tip: i32, bits: Option<i32>) -> Reg {
        Reg {
            id: 1,
            name: "r".to_string(),
            addr: 32,
            n_mb: Some(3),
            tip,
            bits,
            grup: Some(21),
            a_en: true,
            a_no_write: 0,
        }
    }

    #[test]
    fn is_bit_only_for_tip_zero_with_bits() {
        let r_bool = mk_reg(0, Some(0));
        assert!(r_bool.is_bit(), "tip=0 with bits must be bit register");

        let r_f32_with_bits = mk_reg(5, Some(0));
        assert!(
            !r_f32_with_bits.is_bit(),
            "tip=5 must not be treated as bit even if bits is set"
        );
        assert!(r_f32_with_bits.is_32(), "tip=5 must stay 32-bit numeric");
    }
}
