ALTER TABLE usage_daily
    ADD COLUMN unpriced bigint NOT NULL DEFAULT 0,
    ADD COLUMN images bigint NOT NULL DEFAULT 0,
    ADD COLUMN characters bigint NOT NULL DEFAULT 0,
    ADD COLUMN audio_ms bigint NOT NULL DEFAULT 0,
    ADD COLUMN videos bigint NOT NULL DEFAULT 0,
    ADD COLUMN video_ms bigint NOT NULL DEFAULT 0;
