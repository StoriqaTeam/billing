//! Repos is a module responsible for interacting with access control lists

#[macro_use]
pub mod macros;
pub mod legacy_acl;
pub mod roles_cache;

pub use self::roles_cache::RolesCacheImpl;

use std::collections::HashMap;
use std::rc::Rc;

use errors::Error;
use failure::Error as FailureError;
use failure::Fail;

use stq_types::{BillingRole, UserId};

use super::legacy_acl::{Acl, CheckScope};

use models::authorization::*;

pub fn check<T>(
    acl: &Acl<Resource, Action, Scope, FailureError, T>,
    resource: Resource,
    action: Action,
    scope_checker: &CheckScope<Scope, T>,
    obj: Option<&T>,
) -> Result<(), FailureError> {
    acl.allows(resource, action, scope_checker, obj).and_then(|allowed| {
        if allowed {
            Ok(())
        } else {
            Err(Error::Forbidden
                .context(format!("Denied request to do {:?} on {:?}", action, resource))
                .into())
        }
    })
}

/// ApplicationAcl contains main logic for manipulation with recources
#[derive(Clone)]
pub struct ApplicationAcl {
    acls: Rc<HashMap<BillingRole, Vec<Permission>>>,
    roles: Vec<BillingRole>,
    user_id: UserId,
}

impl ApplicationAcl {
    pub fn new(roles: Vec<BillingRole>, user_id: UserId) -> Self {
        let mut hash = ::std::collections::HashMap::new();
        hash.insert(
            BillingRole::Superuser,
            vec![
                permission!(Resource::OrderInfo),
                permission!(Resource::Merchant),
                permission!(Resource::UserRoles),
                permission!(Resource::Invoice),
            ],
        );
        hash.insert(
            BillingRole::User,
            vec![
                permission!(Resource::UserRoles, Action::Read, Scope::Owned),
                permission!(Resource::Invoice, Action::Read, Scope::Owned),
                permission!(Resource::Invoice, Action::Write, Scope::Owned),
                permission!(Resource::OrderInfo, Action::Write, Scope::Owned),
                permission!(Resource::OrderInfo, Action::Read, Scope::Owned),
            ],
        );
        hash.insert(
            BillingRole::StoreManager,
            vec![
                permission!(Resource::OrderInfo, Action::Read, Scope::Owned),
                permission!(Resource::Merchant, Action::Read, Scope::Owned),
                permission!(Resource::UserRoles, Action::Read, Scope::Owned),
            ],
        );
        ApplicationAcl {
            acls: Rc::new(hash),
            roles,
            user_id,
        }
    }
}

impl<T> Acl<Resource, Action, Scope, FailureError, T> for ApplicationAcl {
    fn allows(
        &self,
        resource: Resource,
        action: Action,
        scope_checker: &CheckScope<Scope, T>,
        obj: Option<&T>,
    ) -> Result<bool, FailureError> {
        let empty: Vec<Permission> = Vec::new();
        let user_id = &self.user_id;
        let hashed_acls = self.acls.clone();
        let acls = self
            .roles
            .iter()
            .flat_map(|role| hashed_acls.get(role).unwrap_or(&empty))
            .filter(|permission| (permission.resource == resource) && ((permission.action == action) || (permission.action == Action::All)))
            .filter(|permission| scope_checker.is_in_scope(*user_id, &permission.scope, obj));

        Ok(acls.count() > 0)
    }
}

#[cfg(test)]
mod tests {
    use repos::legacy_acl::{Acl, CheckScope};
    use stq_static_resources::OrderState;
    use stq_types::*;

    use models::*;
    use repos::*;

    fn create_order() -> OrderInfo {
        OrderInfo {
            id: OrderInfoId::new(),
            order_id: OrderId::new(),
            customer_id: UserId(1),
            store_id: StoreId(1),
            saga_id: SagaId::new(),
            status: OrderState::New,
        }
    }

    #[derive(Default)]
    struct ScopeChecker;

    impl CheckScope<Scope, OrderInfo> for ScopeChecker {
        fn is_in_scope(&self, _user_id: UserId, scope: &Scope, _obj: Option<&OrderInfo>) -> bool {
            match *scope {
                Scope::All => true,
                Scope::Owned => false,
            }
        }
    }

    impl CheckScope<Scope, UserRole> for ScopeChecker {
        fn is_in_scope(&self, user_id: UserId, scope: &Scope, obj: Option<&UserRole>) -> bool {
            match *scope {
                Scope::All => true,
                Scope::Owned => {
                    if let Some(user_role) = obj {
                        user_role.user_id == user_id
                    } else {
                        false
                    }
                }
            }
        }
    }

    #[test]
    fn test_super_user_for_users() {
        let acl = ApplicationAcl::new(vec![BillingRole::Superuser], UserId(1232));
        let s = ScopeChecker::default();
        let resource = create_order();

        assert_eq!(acl.allows(Resource::OrderInfo, Action::All, &s, Some(&resource)).unwrap(), true);
        assert_eq!(acl.allows(Resource::OrderInfo, Action::Read, &s, Some(&resource)).unwrap(), true);
        assert_eq!(acl.allows(Resource::OrderInfo, Action::Write, &s, Some(&resource)).unwrap(), true);
    }

    #[test]
    fn test_ordinary_user_for_users() {
        let acl = ApplicationAcl::new(vec![BillingRole::User], UserId(2));
        let s = ScopeChecker::default();
        let resource = create_order();

        assert_eq!(acl.allows(Resource::OrderInfo, Action::All, &s, Some(&resource)).unwrap(), false);
        assert_eq!(acl.allows(Resource::OrderInfo, Action::Read, &s, Some(&resource)).unwrap(), true);
        assert_eq!(acl.allows(Resource::OrderInfo, Action::Write, &s, Some(&resource)).unwrap(), false);
    }

    #[test]
    fn test_super_user_for_user_roles() {
        let acl = ApplicationAcl::new(vec![BillingRole::Superuser], UserId(1232));
        let s = ScopeChecker::default();

        let resource = UserRole {
            id: RoleId::new(),
            user_id: UserId(1),
            name: BillingRole::User,
            data: None,
        };

        assert_eq!(acl.allows(Resource::UserRoles, Action::All, &s, Some(&resource)).unwrap(), true);
        assert_eq!(acl.allows(Resource::UserRoles, Action::Read, &s, Some(&resource)).unwrap(), true);
        assert_eq!(acl.allows(Resource::UserRoles, Action::Write, &s, Some(&resource)).unwrap(), true);
    }

    #[test]
    fn test_user_for_user_roles() {
        let acl = ApplicationAcl::new(vec![BillingRole::User], UserId(2));
        let s = ScopeChecker::default();

        let resource = UserRole {
            id: RoleId::new(),
            user_id: UserId(1),
            name: BillingRole::User,
            data: None,
        };

        assert_eq!(acl.allows(Resource::UserRoles, Action::All, &s, Some(&resource)).unwrap(), false);
        assert_eq!(acl.allows(Resource::UserRoles, Action::Read, &s, Some(&resource)).unwrap(), false);
        assert_eq!(acl.allows(Resource::UserRoles, Action::Write, &s, Some(&resource)).unwrap(), false);
    }
}
