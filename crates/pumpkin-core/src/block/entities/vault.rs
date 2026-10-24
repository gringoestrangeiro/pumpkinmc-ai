use super::BlockEntity;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;
use std::sync::Mutex;
use uuid::Uuid;

pub struct VaultBlockEntity {
    pub position: BlockPos,
    pub config: Mutex<Option<NbtCompound>>,
    pub server_data: Mutex<Option<NbtCompound>>,
    pub shared_data: Mutex<Option<NbtCompound>>,
    pub rewarded_players: Mutex<FxHashSet<Uuid>>,
}

impl BlockEntity for VaultBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let mut rewarded_players = FxHashSet::default();
        if let Some(list) = nbt
            .get_compound("server_data")
            .and_then(|data| data.get_list("rewarded_players"))
        {
            for tag in list {
                if let NbtTag::IntArray(arr) = tag {
                    let bytes: Vec<u8> = arr.iter().flat_map(|part| part.to_be_bytes()).collect();
                    if let Ok(uuid) = Uuid::from_slice(&bytes) {
                        rewarded_players.insert(uuid);
                    }
                }
            }
        }
        Self {
            position,
            config: Mutex::new(nbt.get_compound("config").cloned()),
            server_data: Mutex::new(nbt.get_compound("server_data").cloned()),
            shared_data: Mutex::new(nbt.get_compound("shared_data").cloned()),
            rewarded_players: Mutex::new(rewarded_players),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(cfg) = self.config.lock()
            && let Some(cfg) = cfg.as_ref()
        {
            nbt.put_compound("config", cfg.clone());
        }
        if let Ok(server_data) = self.server_data.lock() {
            let rewarded_players: Vec<NbtTag> = self
                .rewarded_players
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .map(|uuid| {
                    let value = uuid.as_u128();
                    NbtTag::IntArray(vec![
                        (value >> 96) as i32,
                        ((value >> 64) & 0xFFFF_FFFF) as i32,
                        ((value >> 32) & 0xFFFF_FFFF) as i32,
                        (value & 0xFFFF_FFFF) as i32,
                    ])
                })
                .collect();
            if server_data.is_some() || !rewarded_players.is_empty() {
                let mut data = server_data.clone().unwrap_or_default();
                data.put("rewarded_players", NbtTag::List(rewarded_players));
                nbt.put_compound("server_data", data);
            }
        }
        if let Ok(shared) = self.shared_data.lock()
            && let Some(shared) = shared.as_ref()
        {
            nbt.put_compound("shared_data", shared.clone());
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(shared) = self.shared_data.try_lock()
            && let Some(ref shared) = *shared
        {
            nbt.put_compound("shared_data", shared.clone());
        }
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl VaultBlockEntity {
    pub const ID: &'static str = "minecraft:vault";

    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            config: Mutex::new(None),
            server_data: Mutex::new(None),
            shared_data: Mutex::new(None),
            rewarded_players: Mutex::new(FxHashSet::default()),
        }
    }

    pub fn has_rewarded(&self, player_id: &Uuid) -> bool {
        self.rewarded_players
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(player_id)
    }

    pub fn mark_rewarded(&self, player_id: Uuid) {
        self.rewarded_players
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(player_id);
    }
}
