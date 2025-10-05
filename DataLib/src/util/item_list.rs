use crate::types::ids::{TypeID, GroupID, CategoryID};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TypeList<'a> {
    pub included_types: &'a [TypeID],
    pub excluded_types: &'a [TypeID],
    pub included_groups: &'a [GroupID],
    pub excluded_groups: &'a [GroupID],
    pub included_categories: &'a [CategoryID],
    pub excluded_categories: &'a [CategoryID],
}

impl<'a> TypeList<'a> {
    pub const fn empty() -> Self {
        TypeList {
            included_types: &[],
            excluded_types: &[],
            included_groups: &[],
            excluded_groups: &[],
            included_categories: &[],
            excluded_categories: &[],
        }
    }

    pub fn includes_type(&self, type_id: TypeID, group_id: GroupID, category_id: CategoryID) -> bool {
        (
            self.included_types.contains(&type_id)
                || self.included_groups.contains(&group_id)
                || self.included_categories.contains(&category_id)
        ) && !(
            self.excluded_types.contains(&type_id)
                || self.excluded_groups.contains(&group_id)
                || self.excluded_categories.contains(&category_id)
        )
    }

    pub fn includes<F: FnOnce(TypeID) -> (GroupID, CategoryID)>(&self, type_id: TypeID, f: F) -> bool {
        let (group_id, category_id) = f(type_id);
        self.includes_type(type_id, group_id, category_id)
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn flatten<'b,
        FT: Fn(TypeID) -> (GroupID, CategoryID),
        FG: Fn(GroupID) -> (CategoryID, &'b [TypeID]),
        FC: Fn(CategoryID) -> &'b [GroupID]
    >(&'b self, type_info: FT, group_info: FG, category_info: FC) -> Vec<TypeID> {
        let mut buf = Vec::with_capacity(self.included_types.len());

        for type_id in self.included_types {
            if !self.excluded_types.contains(type_id) {
                let (group, category) = type_info(*type_id);
                if !(self.excluded_groups.contains(&group) || self.excluded_categories.contains(&category)) {
                    buf.push(*type_id);
                }
            }
        }

        for group in self.included_groups {
            if !self.excluded_groups.contains(group) {
                let (category, types) = group_info(*group);
                if !self.excluded_categories.contains(&category) {
                    for type_id in types {
                        if !self.excluded_types.contains(type_id) {
                            buf.push(*type_id);
                        }
                    }
                }
            }
        }

        for category in self.included_categories {
            if !self.excluded_categories.contains(category) {
                for group in category_info(*category) {
                    if !self.excluded_groups.contains(group) {
                        let (_, types) = group_info(*group);
                        for type_id in types {
                            if !self.excluded_types.contains(type_id) {
                                buf.push(*type_id);
                            }
                        }
                    }
                }
            }
        }

        buf
    }
}
