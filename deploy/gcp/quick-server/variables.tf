variable "project" {
  description = "The Google Cloud project."
  type        = string
}

variable "region" {
  description = "The region of the instance."
  type        = string
  default     = "europe-west3"
}

variable "zone" {
  description = "The zone of the instance."
  type        = string
  default     = "europe-west3-a"
}

variable "name" {
  description = "The name of the instance and its firewall rules."
  type        = string
  default     = "kaeso"
}

variable "domain" {
  description = "The domain Kaeso answers on, such as kaeso.example.com; point it at the address, or give the DNS zone below."
  type        = string
  validation {
    condition     = can(regex("^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)+$", var.domain))
    error_message = "The domain must be a lowercase host name."
  }
}

variable "kaeso_version" {
  description = "The Kaeso release to install, such as 0.5.6; empty installs the newest stable release."
  type        = string
  default     = ""
}

variable "machine_type" {
  description = "A machine type with at least 2 vCPUs and 4 GB of memory."
  type        = string
  default     = "e2-medium"
}

variable "disk_size_gb" {
  description = "Boot disk in GB; request logs grow it."
  type        = number
  default     = 40
}

variable "ssh_source_ranges" {
  description = "Who may reach SSH; narrow it to your address."
  type        = list(string)
  default     = ["0.0.0.0/0"]
}

variable "dns_zone" {
  description = "A Cloud DNS managed zone of the domain, to create its record; empty leaves DNS to you."
  type        = string
  default     = ""
}
