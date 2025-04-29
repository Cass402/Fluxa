variable "aws_region" {
  description = "AWS region to deploy resources"
  default     = "us-east-1"
  type        = string
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC"
  default     = "10.0.0.0/16"
  type        = string
}

variable "availability_zones" {
  description = "List of availability zones to use"
  default     = ["us-east-1a", "us-east-1b"]
  type        = list(string)
}

variable "public_subnet_cidrs" {
  description = "CIDR blocks for public subnets"
  default     = ["10.0.1.0/24", "10.0.2.0/24"]
  type        = list(string)
}

variable "private_subnet_cidrs" {
  description = "CIDR blocks for private subnets"
  default     = ["10.0.3.0/24", "10.0.4.0/24"]
  type        = list(string)
}

variable "alarm_email" {
  description = "Email address to send monitoring alerts to"
  default     = "devops@fluxa.io"
  type        = string
}