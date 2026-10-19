use super::BlockEntity;
use super::{BlockEntityComponents, custom_name_component, custom_name_from_stack};
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{BannerPatternsImpl, DataComponentImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use std::sync::Mutex;

pub struct BannerBlockEntity {
    pub position: BlockPos,
    pub custom_name: Mutex<Option<String>>,
    pub patterns: Mutex<Option<Vec<NbtTag>>>,
}

impl BlockEntity for BannerBlockEntity {
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
        let custom_name = nbt.get_string("CustomName").map(ToString::to_string);
        let patterns = nbt.get_list("patterns").map(<[_]>::to_vec);
        Self {
            position,
            custom_name: Mutex::new(custom_name),
            patterns: Mutex::new(patterns),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(name) = self.custom_name.lock()
            && let Some(name) = name.as_ref()
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Ok(pats) = self.patterns.lock()
            && let Some(pats) = pats.as_ref()
        {
            nbt.put_list("patterns", pats.clone());
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(name) = self.custom_name.try_lock()
            && let Some(ref name) = *name
        {
            nbt.put_string("CustomName", name.clone());
        }
        if let Ok(patterns) = self.patterns.try_lock()
            && let Some(ref pats) = *patterns
        {
            nbt.put_list("patterns", pats.clone());
        }
        Some(nbt)
    }

    fn collect_components(&self) -> BlockEntityComponents {
        let mut components = Vec::new();

        if let Ok(patterns) = self.patterns.lock()
            && let Some(patterns) = patterns.as_ref()
            && let Some(patterns) = BannerPatternsImpl::read_data(&NbtTag::List(patterns.clone()))
        {
            components.push((
                DataComponent::BannerPatterns,
                Box::new(patterns) as Box<dyn DataComponentImpl>,
            ));
        }

        if let Ok(name) = self.custom_name.lock()
            && let Some(name) = name.as_ref()
        {
            components.push(custom_name_component(name));
        }

        components
    }

    fn apply_item_components(&self, stack: &ItemStack) {
        if let Some(patterns) = stack.get_data_component::<BannerPatternsImpl>()
            && let NbtTag::List(patterns) = patterns.write_data()
        {
            *self
                .patterns
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(patterns);
        }

        if let Some(name) = custom_name_from_stack(stack) {
            *self
                .custom_name
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(name);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BannerBlockEntity {
    pub const ID: &'static str = "minecraft:banner";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            custom_name: Mutex::new(None),
            patterns: Mutex::new(None),
        }
    }
}
