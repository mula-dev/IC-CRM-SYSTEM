#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Customer {
    id: u64,
    name: String,
    email: String,
    phone: String,
    created_at: u64,
}

impl Storable for Customer {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Customer {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

// Use "Interaction" instead of "Message" to represent customer interactions
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Interaction {
    id: u64,
    customer_id: u64,
    interaction_type: String, // e.g., "Email", "Call", "Meeting", etc.
    content: String,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Interaction {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Interaction {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static CUSTOMER_ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static INTERACTION_STORAGE: RefCell<StableBTreeMap<u64, Interaction, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));

    static CUSTOMER_STORAGE: RefCell<StableBTreeMap<u64, Customer, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct InteractionPayload {
    customer_id: u64,
    interaction_type: String,
    content: String,
}

// Helper method to get an interaction by id. Used in get_interaction/update_interaction
fn _get_interaction(id: &u64) -> Option<Interaction> {
    INTERACTION_STORAGE.with(|service| service.borrow().get(id))
}

#[ic_cdk::query]
fn get_interaction(id: u64) -> Result<Interaction, Error> {
    match _get_interaction(&id) {
        Some(interaction) => Ok(interaction),
        None => Err(Error::NotFound {
            msg: format!("an interaction with id={} not found", id),
        }),
    }
}

// Validate interaction payload
fn is_valid_interaction_payload(payload: &InteractionPayload) -> bool {
    // Add your validation logic here
    // For simplicity, let's assume any non-empty interaction_type and content are valid
    !payload.interaction_type.is_empty() && !payload.content.is_empty()
}

#[ic_cdk::update]
fn add_interaction(payload: InteractionPayload) -> Result<Interaction, Error> {
    // Validate interaction payload
    if !is_valid_interaction_payload(&payload) {
        // Return an error if the input is invalid
        return Err(Error::InvalidInput {
            msg: "Invalid interaction payload".to_string(),
        });
    }

    let id = CUSTOMER_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment interaction id counter");

    let interaction = Interaction {
        id,
        customer_id: payload.customer_id,
        interaction_type: payload.interaction_type,
        content: payload.content,
        created_at: time(),
        updated_at: None,
    };

    do_insert_interaction(&interaction);
    Ok(interaction)
}

#[ic_cdk::update]
fn update_interaction(id: u64, payload: InteractionPayload) -> Result<Interaction, Error> {
    // Validate interaction payload
    if !is_valid_interaction_payload(&payload) {
        // Return an error if the input is invalid
        return Err(Error::InvalidInput {
            msg: "Invalid interaction payload".to_string(),
        });
    }

    match INTERACTION_STORAGE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut interaction) => {
            interaction.interaction_type = payload.interaction_type;
            interaction.content = payload.content;
            interaction.updated_at = Some(time());
            do_insert_interaction(&interaction);
            Ok(interaction.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update an interaction with id={}. Interaction not found",
                id
            ),
        }),
    }
}

// Update the function names and variables to reflect the CRM logic
fn do_insert_interaction(interaction: &Interaction) {
    INTERACTION_STORAGE.with(|service| service.borrow_mut().insert(interaction.id, interaction.clone()));
}

// Helper method to get a customer by id. Used in get_customer/update_customer
fn _get_customer(id: &u64) -> Option<Customer> {
    CUSTOMER_STORAGE.with(|service| service.borrow().get(id))
}

#[ic_cdk::query]
fn get_customer(id: u64) -> Result<Customer, Error> {
    match _get_customer(&id) {
        Some(customer) => Ok(customer),
        None => Err(Error::NotFound {
            msg: format!("a customer with id={} not found", id),
        }),
    }
}

// Add validation functions for email and phone number
fn is_valid_email(email: &str) -> bool {
    // Implement your email validation logic here
    // For simplicity, let's assume any string with '@' is considered a valid email
    email.contains('@')
}

fn is_valid_phone(phone: &str) -> bool {
    // Implement your phone number validation logic here
    // For simplicity, let's assume any string with digits is considered a valid phone number
    phone.chars().any(char::is_numeric)
}

#[ic_cdk::update]
fn add_customer(name: String, email: String, phone: String) -> Result<Customer, Error> {
    // Validate email and phone
    if !is_valid_email(&email) || !is_valid_phone(&phone) {
        // Return an error if the input is invalid
        return Err(Error::InvalidInput {
            msg: "Invalid email or phone format".to_string(),
        });
    }

    let id = CUSTOMER_ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment customer id counter");

    let customer = Customer {
        id,
        name,
        email,
        phone,
        created_at: time(),
    };

    do_insert_customer(&customer);
    Ok(customer)
}

// Update the function names and variables to reflect the CRM logic
fn do_insert_customer(customer: &Customer) {
    CUSTOMER_STORAGE.with(|service| service.borrow_mut().insert(customer.id, customer.clone()));
}

#[ic_cdk::update]
fn delete_interaction(id: u64) -> Result<Interaction, Error> {
    match INTERACTION_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(interaction) => Ok(interaction),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete an interaction with id={}. Interaction not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn update_customer(id: u64, name: String, email: String, phone: String) -> Result<Customer, Error> {
    // Validate email and phone
    if !is_valid_email(&email) || !is_valid_phone(&phone) {
        // Return an error if the input is invalid
        return Err(Error::InvalidInput {
            msg: "Invalid email or phone format".to_string(),
        });
    }

    match CUSTOMER_STORAGE.with(|service| service.borrow_mut().get(&id)) {
        Some(mut customer) => {
            customer.name = name;
            customer.email = email;
            customer.phone = phone;
            Ok(customer.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't update a customer with id={}. Customer not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn delete_customer(id: u64) -> Result<Customer, Error> {
    match CUSTOMER_STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(customer) => Ok(customer),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a customer with id={}. Customer not found",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    InvalidInput { msg: String },
}

#[ic_cdk::query]
fn search_customers(
    name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    page_size: u64,
    page_number: u64,
) -> SearchResult<Customer> {
    let all_customers: Vec<Customer> = CUSTOMER_STORAGE
        .with(|service| {
            service
                .borrow()
                .iter()
                .filter(|(_, customer)| {
                    let name_match = name.as_ref().map_or(true, |n| &customer.name == n);
                    let email_match = email.as_ref().map_or(true, |e| &customer.email == e);
                    let phone_match = phone.as_ref().map_or(true, |p| &customer.phone == p);

                    name_match && email_match && phone_match
                })
                .map(|(_, customer)| customer.clone())
                .collect()
        });

    let total_items = all_customers.len();
    let start_index = (page_number - 1) as usize * page_size as usize;
    let end_index = (start_index + page_size as usize).min(total_items);

    let paginated_customers = all_customers[start_index..end_index].to_vec();

    SearchResult {
        total_items,
        items: paginated_customers,
    }
}

// Define a SearchResult struct to hold pagination information
#[derive(candid::CandidType, Serialize, Deserialize)]
struct SearchResult<T> {
    total_items: usize,
    items: Vec<T>,
}

    // need this to generate candid
    ic_cdk::export_candid!();