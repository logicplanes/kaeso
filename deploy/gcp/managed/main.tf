terraform {
  required_version = ">= 1.6"
  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.6"
    }
  }
}

provider "google" {
  project = var.project
  region  = var.region
}

locals {
  gateway_image = "ghcr.io/logicplanes/kaeso-gateway:0.6.0"
  console_image = "ghcr.io/logicplanes/kaeso-console:0.6.0"
  api_paths     = ["/v1/*", "/v1beta/*", "/v2/*", "/platform/*", "/mcp", "/mcp/*", "/oauth/*", "/.well-known/oauth-*"]
  # Cloud Run's own front end and Google's load balancer proxies sit between the client and the gateway.
  trusted_proxies = "169.254.0.0/16,130.211.0.0/22,35.191.0.0/16,fe80::/10,127.0.0.0/8,::1/128"
}

resource "google_project_service" "apis" {
  for_each = toset([
    "run.googleapis.com",
    "compute.googleapis.com",
    "sqladmin.googleapis.com",
    "memorystore.googleapis.com",
    "secretmanager.googleapis.com",
    "servicenetworking.googleapis.com",
    "vpcaccess.googleapis.com",
  ])
  service            = each.key
  disable_on_destroy = false
}

resource "google_compute_network" "kaeso" {
  name                    = "${var.name}-network"
  auto_create_subnetworks = false
  depends_on              = [google_project_service.apis]
}

resource "google_compute_subnetwork" "services" {
  name                     = "${var.name}-services"
  network                  = google_compute_network.kaeso.id
  ip_cidr_range            = "10.83.0.0/20"
  region                   = var.region
  private_ip_google_access = true
}

resource "google_compute_global_address" "private_services" {
  name          = "${var.name}-private-services"
  purpose       = "VPC_PEERING"
  address_type  = "INTERNAL"
  prefix_length = 20
  network       = google_compute_network.kaeso.id
}

resource "google_service_networking_connection" "private_services" {
  network                 = google_compute_network.kaeso.id
  service                 = "servicenetworking.googleapis.com"
  reserved_peering_ranges = [google_compute_global_address.private_services.name]
}

resource "random_password" "database" {
  length  = 32
  special = false
}

resource "google_sql_database_instance" "kaeso" {
  name                = "${var.name}-postgres"
  database_version    = "POSTGRES_17"
  region              = var.region
  deletion_protection = true
  depends_on          = [google_service_networking_connection.private_services]

  settings {
    tier              = var.database_tier
    availability_type = "ZONAL"
    disk_type         = "PD_SSD"
    disk_size         = var.database_disk_gb
    disk_autoresize   = true
    ip_configuration {
      ipv4_enabled                                  = false
      private_network                               = google_compute_network.kaeso.id
      enable_private_path_for_google_cloud_services = true
    }
    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = true
    }
  }
}

resource "google_sql_database" "kaeso" {
  name     = "kaeso"
  instance = google_sql_database_instance.kaeso.name
}

resource "google_sql_user" "kaeso" {
  name     = "kaeso"
  instance = google_sql_database_instance.kaeso.name
  password = random_password.database.result
}

resource "google_memorystore_instance" "kaeso" {
  instance_id    = "${var.name}-valkey"
  location       = var.region
  shard_count    = 1
  engine_version = "VALKEY_8_0"
  node_type      = var.cache_node_type
  desired_psc_auto_connections {
    network    = google_compute_network.kaeso.id
    project_id = var.project
  }
  # Kaeso's counters and budgets must never be evicted.
  engine_configs = {
    maxmemory-policy = "noeviction"
  }
  # Kaeso reaches it only over the private network; it can't present IAM tokens or a private CA.
  transit_encryption_mode = "TRANSIT_ENCRYPTION_DISABLED"
  authorization_mode      = "AUTH_DISABLED"
  depends_on              = [google_project_service.apis]
}

resource "google_vpc_access_connector" "services" {
  name          = "${var.name}-connector"
  region        = var.region
  network       = google_compute_network.kaeso.name
  ip_cidr_range = "10.83.16.0/28"
  depends_on    = [google_project_service.apis]
}

resource "google_secret_manager_secret" "secrets" {
  for_each  = toset(["master-keys", "clickhouse-url", "smtp-password", "postgres-url"])
  secret_id = "${var.name}-${each.key}"
  replication {
    auto {}
  }
  depends_on = [google_project_service.apis]
}

resource "google_secret_manager_secret_version" "master_keys" {
  secret      = google_secret_manager_secret.secrets["master-keys"].id
  secret_data = var.master_keys
}

resource "google_secret_manager_secret_version" "clickhouse_url" {
  secret      = google_secret_manager_secret.secrets["clickhouse-url"].id
  secret_data = var.clickhouse_url
}

resource "google_secret_manager_secret_version" "smtp_password" {
  secret      = google_secret_manager_secret.secrets["smtp-password"].id
  secret_data = var.smtp_password
}

resource "google_secret_manager_secret_version" "postgres_url" {
  secret      = google_secret_manager_secret.secrets["postgres-url"].id
  secret_data = "postgres://kaeso:${random_password.database.result}@${google_sql_database_instance.kaeso.private_ip_address}:5432/kaeso?sslmode=require"
}

resource "google_service_account" "gateway" {
  account_id   = "${var.name}-gateway"
  display_name = "Kaeso gateway"
}

resource "google_secret_manager_secret_iam_member" "gateway" {
  for_each  = google_secret_manager_secret.secrets
  secret_id = each.value.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.gateway.email}"
}

resource "google_cloud_run_v2_service" "gateway" {
  name                = "${var.name}-gateway"
  location            = var.region
  ingress             = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"
  deletion_protection = false
  depends_on          = [google_secret_manager_secret_iam_member.gateway]

  template {
    service_account = google_service_account.gateway.email
    # A model may stream for many minutes; the request may run for an hour at most.
    timeout = "3600s"
    scaling {
      min_instance_count = var.gateway_min_instances
      max_instance_count = var.gateway_max_instances
    }
    vpc_access {
      connector = google_vpc_access_connector.services.id
      egress    = "PRIVATE_RANGES_ONLY"
    }
    containers {
      image = local.gateway_image
      ports {
        container_port = 8080
      }
      resources {
        limits = {
          cpu    = var.gateway_cpu
          memory = var.gateway_memory
        }
        cpu_idle = false
      }
      startup_probe {
        http_get {
          path = "/healthz"
        }
        initial_delay_seconds = 5
        period_seconds        = 5
        failure_threshold     = 24
      }
      liveness_probe {
        http_get {
          path = "/healthz"
        }
        period_seconds = 30
      }
      env {
        name  = "KAESO_PUBLIC_URL"
        value = "https://${var.domain}"
      }
      env {
        name  = "KAESO_METRICS_LISTEN"
        value = "127.0.0.1:9100"
      }
      env {
        name  = "KAESO_TRUSTED_PROXIES"
        value = local.trusted_proxies
      }
      env {
        name  = "KAESO_VALKEY_URL"
        value = "redis://${google_memorystore_instance.kaeso.endpoints[0].connections[0].psc_auto_connection[0].ip_address}:${google_memorystore_instance.kaeso.endpoints[0].connections[0].psc_auto_connection[0].port}"
      }
      env {
        name  = "KAESO_SETUP_EMAIL"
        value = var.setup_email
      }
      env {
        name  = "KAESO_SMTP_HOST"
        value = var.smtp_host
      }
      env {
        name  = "KAESO_SMTP_PORT"
        value = tostring(var.smtp_port)
      }
      env {
        name  = "KAESO_SMTP_USER"
        value = var.smtp_user
      }
      env {
        name  = "KAESO_SMTP_FROM"
        value = var.smtp_from
      }
      env {
        name  = "KAESO_LOG_FORMAT"
        value = "json"
      }
      env {
        name = "KAESO_POSTGRES_URL"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.secrets["postgres-url"].secret_id
            version = "latest"
          }
        }
      }
      env {
        name = "KAESO_MASTER_KEYS"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.secrets["master-keys"].secret_id
            version = "latest"
          }
        }
      }
      env {
        name = "KAESO_CLICKHOUSE_URL"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.secrets["clickhouse-url"].secret_id
            version = "latest"
          }
        }
      }
      env {
        name = "KAESO_SMTP_PASSWORD"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.secrets["smtp-password"].secret_id
            version = "latest"
          }
        }
      }
    }
  }
}

resource "google_cloud_run_v2_service" "console" {
  name                = "${var.name}-console"
  location            = var.region
  ingress             = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"
  deletion_protection = false

  template {
    scaling {
      min_instance_count = 1
      max_instance_count = 3
    }
    containers {
      image = local.console_image
      ports {
        container_port = 3000
      }
      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
      }
    }
  }
}

resource "google_cloud_run_v2_service_iam_member" "public" {
  for_each = {
    gateway = google_cloud_run_v2_service.gateway.name
    console = google_cloud_run_v2_service.console.name
  }
  name     = each.value
  location = var.region
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_compute_region_network_endpoint_group" "services" {
  for_each = {
    gateway = google_cloud_run_v2_service.gateway.name
    console = google_cloud_run_v2_service.console.name
  }
  name                  = "${var.name}-${each.key}-neg"
  region                = var.region
  network_endpoint_type = "SERVERLESS"
  cloud_run {
    service = each.value
  }
}

resource "google_compute_backend_service" "services" {
  for_each              = google_compute_region_network_endpoint_group.services
  name                  = "${var.name}-${each.key}"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  protocol              = "HTTPS"
  # Longer than any model stream, so the load balancer never cuts one off.
  timeout_sec = 3600
  backend {
    group = each.value.id
  }
}

resource "google_compute_url_map" "kaeso" {
  name            = "${var.name}-map"
  default_service = google_compute_backend_service.services["console"].id
  host_rule {
    hosts        = [var.domain]
    path_matcher = "kaeso"
  }
  path_matcher {
    name            = "kaeso"
    default_service = google_compute_backend_service.services["console"].id
    path_rule {
      paths   = local.api_paths
      service = google_compute_backend_service.services["gateway"].id
    }
  }
}

resource "google_compute_managed_ssl_certificate" "kaeso" {
  name = "${var.name}-certificate"
  managed {
    domains = [var.domain]
  }
}

resource "google_compute_target_https_proxy" "kaeso" {
  name             = "${var.name}-https"
  url_map          = google_compute_url_map.kaeso.id
  ssl_certificates = [google_compute_managed_ssl_certificate.kaeso.id]
}

resource "google_compute_global_address" "edge" {
  name = "${var.name}-edge"
}

resource "google_compute_global_forwarding_rule" "https" {
  name                  = "${var.name}-https"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  target                = google_compute_target_https_proxy.kaeso.id
  port_range            = "443"
  ip_address            = google_compute_global_address.edge.id
}

resource "google_compute_url_map" "redirect" {
  name = "${var.name}-redirect"
  default_url_redirect {
    https_redirect         = true
    redirect_response_code = "MOVED_PERMANENTLY_DEFAULT"
    strip_query            = false
  }
}

resource "google_compute_target_http_proxy" "redirect" {
  name    = "${var.name}-http"
  url_map = google_compute_url_map.redirect.id
}

resource "google_compute_global_forwarding_rule" "http" {
  name                  = "${var.name}-http"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  target                = google_compute_target_http_proxy.redirect.id
  port_range            = "80"
  ip_address            = google_compute_global_address.edge.id
}

resource "google_dns_record_set" "kaeso" {
  count        = var.dns_zone == "" ? 0 : 1
  managed_zone = var.dns_zone
  name         = "${var.domain}."
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_global_address.edge.address]
}
