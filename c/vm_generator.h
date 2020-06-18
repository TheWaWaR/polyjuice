#include <evmc/evmc.h>

#include "generator.h"
#include <stdlib.h>

#define _CSAL_RETURN_SYSCALL_NUMBER 3075
#define _CSAL_LOG_SYSCALL_NUMBER 3076
#define _CSAL_SELFDESTRUCT_SYSCALL_NUMBER 3077
#define _CSAL_CALL_SYSCALL_NUMBER 3078

int csal_return(const uint8_t *data, uint32_t data_length) {
  return syscall(_CSAL_RETURN_SYSCALL_NUMBER, data, data_length, 0, 0, 0, 0);
}
int csal_log(const uint8_t *data, uint32_t data_length) {
  return syscall(_CSAL_LOG_SYSCALL_NUMBER, data, data_length, 0, 0, 0, 0);
}
int csal_selfdestruct(const uint8_t *data, uint32_t data_length) {
  return syscall(_CSAL_SELFDESTRUCT_SYSCALL_NUMBER, data, data_length, 0, 0, 0, 0);
}
int csal_call(uint8_t *result_data, const uint8_t *msg_data) {
  return syscall(_CSAL_CALL_SYSCALL_NUMBER, result_data, msg_data, 0, 0, 0, 0);
}

void release_result(const struct evmc_result* result) {
  free((void *)result->output_data);
}


struct evmc_host_context {
  csal_change_t *existing_values;
  csal_change_t *changes;
  evmc_address tx_origin;
  bool destructed;
};

struct evmc_tx_context get_tx_context(struct evmc_host_context* context) {
  struct evmc_tx_context ctx{};
  ctx.tx_origin = context->tx_origin;
  return ctx;
}

bool account_exists(struct evmc_host_context* context,
                    const evmc_address* address) {
  return true;
}

evmc_bytes32 get_storage(struct evmc_host_context* context,
                         const evmc_address* address,
                         const evmc_bytes32* key) {
  evmc_bytes32 value{};
  int ret;
  ret = csal_change_fetch(context->changes, key->bytes, value.bytes);
  if (ret != 0) {
    ret = csal_change_fetch(context->existing_values, key->bytes, value.bytes);
  }
  return value;
}

enum evmc_storage_status set_storage(struct evmc_host_context* context,
                                     const evmc_address* address,
                                     const evmc_bytes32* key,
                                     const evmc_bytes32* value) {
  /* int _ret; */
  csal_change_insert(context->existing_values, key->bytes, value->bytes);
  csal_change_insert(context->changes, key->bytes, value->bytes);
  return EVMC_STORAGE_ADDED;
}

size_t get_code_size(struct evmc_host_context* context,
                     const evmc_address* address) {
  return 0;
}

evmc_bytes32 get_code_hash(struct evmc_host_context* context,
                           const evmc_address* address) {
  evmc_bytes32 hash{};
  return hash;
}

size_t copy_code(struct evmc_host_context* context,
                 const evmc_address* address,
                 size_t code_offset,
                 uint8_t* buffer_data,
                 size_t buffer_size) {
  return 0;
}

evmc_uint256be get_balance(struct evmc_host_context* context,
                           const evmc_address* address) {
  // TODO: how to return balance?
  evmc_uint256be balance{};
  return balance;
}

void selfdestruct(struct evmc_host_context* context,
                  const evmc_address* address,
                  const evmc_address* beneficiary) {
  context->destructed = true;
  csal_selfdestruct(beneficiary->bytes, 20);
}

struct evmc_result call(struct evmc_host_context* context,
                        const struct evmc_message* msg) {
  uint8_t result_data[10 * 1024];
  uint8_t msg_data[10 * 1024];
  uint8_t *msg_ptr = msg_data;

  *msg_ptr = (uint8_t)msg->kind;
  msg_ptr += 1;
  memcpy(msg_ptr, ((uint8_t *)&msg->flags), 4);
  msg_ptr += 4;
  memcpy(msg_ptr, ((uint8_t *)&msg->depth), 4);
  msg_ptr += 4;
  memcpy(msg_ptr, ((uint8_t *)&msg->gas), 8);
  msg_ptr += 8;
  memcpy(msg_ptr, &msg->destination.bytes, 20);
  msg_ptr += 20;
  memcpy(msg_ptr, &msg->sender.bytes, 20);
  msg_ptr += 20;

  uint32_t input_size = (uint32_t) msg->input_size;
  memcpy(msg_ptr, ((uint8_t *)&input_size), 4);
  msg_ptr += 4;
  memcpy(msg_ptr, msg->input_data, msg->input_size);
  msg_ptr += msg->input_size;
  memcpy(msg_ptr, &msg->value.bytes, 32);
  msg_ptr += 32;
  memcpy(msg_ptr, &msg->create2_salt.bytes, 32);
  csal_call(result_data, msg_data);

  uint8_t *result_ptr = result_data;
  int32_t output_size_32 = *((int32_t *)result_ptr);
  result_ptr += 4;

  size_t output_size = (size_t)output_size_32;
  uint8_t *output_data = (uint8_t *)malloc(output_size);
  memcpy(output_data, result_ptr, output_size);
  result_ptr += output_size;

  evmc_address create_address{};
  memcpy(&create_address.bytes, result_ptr, 20);
  result_ptr += 20;


  struct evmc_result res = { EVMC_SUCCESS, msg->gas, output_data, output_size, release_result, create_address };
  memset(res.padding, 0, 4);
  return res;
}

void emit_log(struct evmc_host_context* context,
              const evmc_address* address,
              const uint8_t* data,
              size_t data_size,
              const evmc_bytes32 topics[],
              size_t topics_count) {
  uint8_t buffer[2048];
  uint32_t offset = 0;
  uint32_t the_data_size = (uint32_t)data_size;
  uint32_t the_topics_count = (uint32_t)topics_count;
  size_t i;
  for (i = 0; i < sizeof(uint32_t); i++) {
    buffer[offset++] = *((uint8_t *)(&the_data_size) + i);
  }
  for (i = 0; i < data_size; i++) {
    buffer[offset++] = data[i];
  }
  for (i = 0; i < sizeof(uint32_t); i++) {
    buffer[offset++] = *((uint8_t *)(&the_topics_count) + i);
  }
  for (i = 0; i < topics_count; i++) {
    const evmc_bytes32 *topic = topics + i;
    for (size_t j = 0; j < 32; j++) {
      buffer[offset++] = topic->bytes[j];
    }
  }
  csal_log(buffer, offset);
}


inline int verify_params(const uint8_t *signature_data,
                         const uint8_t call_kind,
                         const uint32_t flags,
                         const uint32_t depth,
                         const evmc_address *tx_origin,
                         const evmc_address *sender,
                         const evmc_address *destination,
                         const uint32_t code_size,
                         const uint8_t *code_data,
                         const uint32_t input_size,
                         const uint8_t *input_data) {
  /* Do nothing */
  return 0;
}

inline void context_init(struct evmc_host_context* context,
                         struct evmc_vm *vm,
                         struct evmc_host_interface *interface,
                         evmc_address tx_origin,
                         csal_change_t *existing_values,
                         csal_change_t *changes) {
  context->existing_values = existing_values;
  context->changes = changes;
  context->tx_origin = tx_origin;
  context->destructed = false;
}

inline void return_result(const struct evmc_message *_msg, const struct evmc_result *res) {
  if (res->status_code == EVMC_SUCCESS) {
    csal_return(res->output_data, res->output_size);
  }
}

inline int verify_result(struct evmc_host_context* context,
                         const struct evmc_message *msg,
                         const struct evmc_result *res,
                         const uint8_t *return_data,
                         const size_t return_data_size,
                         const evmc_address *beneficiary) {
  /* Do nothing */
  return 0;
}
