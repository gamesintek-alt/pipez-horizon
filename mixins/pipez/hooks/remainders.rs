fn pipez_remainders(value: Vec<Option<ItemStack>>, recipe: &str, grid: &[Option<ItemStack>], size: usize) -> Vec<Option<ItemStack>> {
if recipe.ends_with("000_settings"){crate::crafting::pipez_crafting::remainders(grid,size).unwrap_or(value)}else{value}
}
