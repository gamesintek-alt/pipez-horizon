fn pipez_craft(value: Option<Option<ItemStack>>, input: &Input, recipe: &Recipe) -> Option<Option<ItemStack>> {

let marker=match recipe {Recipe::Shaped{result,..}|Recipe::Shapeless{result,..}=>result.create().custom_data().is_some_and(|c|c.contains_key(&"pipez_settings_recipe".into())),_=>false};
if !marker{return value;}
let grid:Vec<Option<ItemStack>>=input.cells.iter().map(|s|s.cloned()).collect();
crate::crafting::pipez_crafting::recipe(&grid).map(|(_,item,_)|Some(item))

}
