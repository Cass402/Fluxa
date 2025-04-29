terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
  backend "s3" {
    bucket = "fluxa-terraform-state-dev"
    key    = "state/terraform.tfstate"
    region = "us-east-1"
  }
}

provider "aws" {
  region = var.aws_region
  default_tags {
    tags = {
      Environment = "development"
      Project     = "fluxa"
      ManagedBy   = "terraform"
    }
  }
}

# Network configuration
module "vpc" {
  source = "../modules/vpc"
  
  environment       = "development"
  vpc_cidr          = var.vpc_cidr
  availability_zones = var.availability_zones
  public_subnet_cidrs  = var.public_subnet_cidrs
  private_subnet_cidrs = var.private_subnet_cidrs
}

# Monitoring infrastructure
module "monitoring" {
  source = "../modules/monitoring"
  
  environment = "development"
  vpc_id      = module.vpc.vpc_id
  subnet_ids  = module.vpc.private_subnet_ids
  
  alarm_email = var.alarm_email
}

# Security groups
module "security_groups" {
  source = "../modules/security"
  
  environment = "development"
  vpc_id      = module.vpc.vpc_id
}

# Output important values
output "vpc_id" {
  value = module.vpc.vpc_id
}

output "public_subnet_ids" {
  value = module.vpc.public_subnet_ids
}

output "private_subnet_ids" {
  value = module.vpc.private_subnet_ids
}

output "monitoring_dashboard_url" {
  value = module.monitoring.dashboard_url
}