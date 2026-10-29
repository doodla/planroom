//! The level ladder: the only order levels have. Levels are pack data (`LevelId` is an open set
//! with no order), so the kernel ranks them by the vocabulary it is given: the kernel's `intent`
//! apart, first, and then the chain the packs declare, top to finest.

use planroom_contract::{LevelId, LevelVocab, Vocabulary};

/// A vocabulary's levels in order: `intent`, then the chain top to finest.
#[derive(Debug, Clone)]
pub struct Ladder<'v> {
    levels: &'v [LevelVocab],
    /// The chain: every level that isn't apart, in the vocabulary's order.
    chain: Vec<&'v LevelId>,
    /// `intent`, which ranks first whether or not the vocabulary lists it.
    intent: LevelId,
}

impl<'v> Ladder<'v> {
    /// The ladder `vocabulary.levels` declares: its entries marked `apart` are intent; the rest,
    /// in order, are the chain.
    pub fn of(vocabulary: &'v Vocabulary) -> Self {
        let levels = vocabulary.levels.as_slice();
        let chain = levels.iter().filter(|l| !l.apart && !l.id.is_intent()).map(|l| &l.id).collect();
        Ladder { levels, chain, intent: LevelId::intent() }
    }

    /// Where `level` sits: intent 0, the chain 1 to n from the top to the finest, and a level
    /// the ladder doesn't hold n + 1, below every other.
    pub fn rank(&self, level: &LevelId) -> u8 {
        if self.is_apart(level) {
            return 0;
        }
        let at = self.chain.iter().position(|l| *l == level).unwrap_or(self.chain.len());
        u8::try_from(at + 1).unwrap_or(u8::MAX)
    }

    /// The chain's first level; intent when there is no chain.
    pub fn top(&self) -> &LevelId {
        self.chain.first().copied().unwrap_or(&self.intent)
    }

    /// The chain's last level; intent when there is no chain.
    pub fn finest(&self) -> &LevelId {
        self.chain.last().copied().unwrap_or(&self.intent)
    }

    /// The level one step finer: intent opens the top, the chain steps finer, and the finest
    /// (or a level the ladder doesn't hold) stays at the finest.
    pub fn below(&self, level: &LevelId) -> &LevelId {
        if self.is_apart(level) {
            return self.top();
        }
        match self.chain.iter().position(|l| *l == level) {
            Some(at) if at + 1 < self.chain.len() => self.chain[at + 1],
            _ => self.finest(),
        }
    }

    /// The level one step coarser: none at the top and at intent; the finest for a level the
    /// ladder doesn't hold, which ranks below it.
    pub fn above(&self, level: &LevelId) -> Option<&LevelId> {
        if self.is_apart(level) {
            return None;
        }
        match self.chain.iter().position(|l| *l == level) {
            Some(at) => at.checked_sub(1).map(|up| self.chain[up]),
            None => self.chain.last().copied(),
        }
    }

    /// Whether `level` stands apart from the chain: `intent`, listed or not, or a level the
    /// vocabulary marks apart.
    pub fn is_apart(&self, level: &LevelId) -> bool {
        level.is_intent() || self.levels.iter().any(|l| l.apart && l.id == *level)
    }

    /// Whether the ladder holds `level`: intent always, and every level the vocabulary lists.
    pub fn holds(&self, level: &LevelId) -> bool {
        level.is_intent() || self.levels.iter().any(|l| l.id == *level)
    }

    /// The order a lookup tries levels in: `level` itself, then each coarser level nearest
    /// first, down to intent, then each finer level nearest first.
    pub fn order_from<'a>(&'a self, level: &'a LevelId) -> Vec<&'a LevelId> {
        let all: Vec<&LevelId> = std::iter::once(&self.intent).chain(self.chain.iter().copied()).collect();
        let at = usize::from(self.rank(level)).min(all.len());
        let mut order = Vec::with_capacity(all.len() + 1);
        order.push(level);
        order.extend(all[..at].iter().rev().copied().filter(|l| *l != level));
        order.extend(all[at..].iter().copied().filter(|l| *l != level));
        order
    }

    /// The vocabulary's levels as it lists them.
    pub fn levels(&self) -> &'v [LevelVocab] {
        self.levels
    }
}

/// An invented ladder for the kernel's unit tests, so none of them leans on a pack's levels:
/// `intent`, then `realm`, `region`, `site` and `room`, top to finest.
#[cfg(test)]
pub(crate) mod testing {
    use planroom_contract::{LevelId, LevelVocab, Vocabulary};

    pub(crate) const CHAIN: [&str; 4] = ["realm", "region", "site", "room"];

    /// A vocabulary that declares only the invented ladder.
    pub(crate) fn vocabulary() -> Vocabulary {
        let level = |id: &str, apart: bool| LevelVocab {
            id: LevelId::from(id),
            label: id.into(),
            one: "{count} thing".into(),
            many: "{count} things".into(),
            apart,
            pack: "test".into(),
        };
        let mut levels = vec![level(LevelId::INTENT, true)];
        levels.extend(CHAIN.iter().map(|id| level(id, false)));
        Vocabulary { levels, ..Vocabulary::default() }
    }

    pub(crate) fn intent() -> LevelId {
        LevelId::intent()
    }
    pub(crate) fn realm() -> LevelId {
        LevelId::from(CHAIN[0])
    }
    pub(crate) fn region() -> LevelId {
        LevelId::from(CHAIN[1])
    }
    pub(crate) fn site() -> LevelId {
        LevelId::from(CHAIN[2])
    }
    pub(crate) fn room() -> LevelId {
        LevelId::from(CHAIN[3])
    }
}
