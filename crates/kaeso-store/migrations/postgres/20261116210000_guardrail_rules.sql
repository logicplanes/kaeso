ALTER TABLE guardrails
    ADD COLUMN kind text NOT NULL DEFAULT 'http' CHECK (kind IN ('http', 'rules')),
    ADD COLUMN rules jsonb CHECK (jsonb_typeof(rules) = 'object'),
    ALTER COLUMN url DROP NOT NULL,
    ALTER COLUMN secret_ciphertext DROP NOT NULL,
    ALTER COLUMN secret_nonce DROP NOT NULL,
    ALTER COLUMN secret_wrapped_key DROP NOT NULL,
    ALTER COLUMN secret_key_id DROP NOT NULL,
    ADD CONSTRAINT guardrails_kind CHECK (
        COALESCE(
            (kind = 'http' AND url IS NOT NULL AND rules IS NULL
                AND num_nonnulls(secret_ciphertext, secret_nonce, secret_wrapped_key, secret_key_id) = 4)
            OR (kind = 'rules' AND url IS NULL AND rules IS NOT NULL
                AND num_nonnulls(secret_ciphertext, secret_nonce, secret_wrapped_key, secret_key_id) = 0),
            false
        )
    );
