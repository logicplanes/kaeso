variable "project" {
  description = "The Google Cloud project."
  type        = string
}

variable "region" {
  description = "The region of every regional resource."
  type        = string
  default     = "europe-west3"
}

variable "name" {
  description = "The prefix of every resource's name."
  type        = string
  default     = "kaeso"
}

variable "domain" {
  description = "The domain Kaeso answers on, such as kaeso.example.com."
  type        = string
  validation {
    condition     = can(regex("^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$", var.domain))
    error_message = "The domain must be a lowercase host name."
  }
}

variable "dns_zone" {
  description = "A Cloud DNS managed zone of the domain, to create its record; empty leaves DNS to you."
  type        = string
  default     = ""
}

variable "master_keys" {
  description = "The master keys as id:base64, from kaeso master-key generate; keep a copy in a password manager."
  type        = string
  sensitive   = true
}

variable "setup_email" {
  description = "Where the gateway emails the one-time setup link until the first user exists."
  type        = string
}

variable "clickhouse_url" {
  description = "A ClickHouse HTTP URL with user and password for request logs, such as ClickHouse Cloud; empty keeps request logs off."
  type        = string
  sensitive   = true
  default     = ""
}

variable "smtp_host" {
  description = "The SMTP server for sign-in links, invitations and alerts."
  type        = string
}

variable "smtp_port" {
  type    = number
  default = 587
}

variable "smtp_user" {
  type    = string
  default = ""
}

variable "smtp_password" {
  type      = string
  sensitive = true
  default   = ""
}

variable "smtp_from" {
  description = "The sender, such as Kaeso <kaeso@example.com>."
  type        = string
}

variable "gateway_cpu" {
  description = "CPU of a gateway instance."
  type        = string
  default     = "2"
}

variable "gateway_memory" {
  description = "Memory of a gateway instance."
  type        = string
  default     = "4Gi"
}

variable "gateway_min_instances" {
  description = "Gateway instances kept warm; they share limits, caches and route cooldowns through Valkey."
  type        = number
  default     = 1
}

variable "gateway_max_instances" {
  type    = number
  default = 10
}

variable "database_tier" {
  description = "The Cloud SQL machine tier."
  type        = string
  default     = "db-custom-1-3840"
}

variable "database_disk_gb" {
  type    = number
  default = 20
}

variable "cache_node_type" {
  description = "The Memorystore for Valkey node type."
  type        = string
  default     = "SHARED_CORE_NANO"
}
