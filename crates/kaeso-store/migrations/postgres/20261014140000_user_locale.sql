ALTER TABLE users
    ADD COLUMN locale text CHECK (locale IN ('en', 'de', 'zh-CN'));
